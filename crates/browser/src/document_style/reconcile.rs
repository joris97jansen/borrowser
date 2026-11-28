//! Fallible preparation reads the old inventory; commit only moves its owners.
use super::participation::{HostEligibility, LinkLabel, contributes};
use super::{
    BodyReservation, BrowserUrl, DocumentStyleSet, HashMap, Node, ParseOptions, ResourceAddress,
    ResourceCapability, ResourceKind, ResourceOperation, StylesheetParse,
    StylesheetReconcileResult, StylesheetSlot, StylesheetSlotId, StylesheetSlotSource,
    StylesheetSlotState, parse_stylesheet_with_options,
};
#[cfg(test)]
use super::{Id, StylesheetRetentionFailure};
use crate::stylesheet_fetch::{
    ATTACHMENTS, BODY_BYTES, METADATA_BYTES, ORDINARY_NODES, SOURCE_BYTES, StylesheetFetchFailure,
    StylesheetFetchResult, StylesheetRequestEvidence, StylesheetSourceFailure, try_string,
};
#[cfg(test)]
use crate::stylesheet_fetch::{
    AllocationBoundary, BODY_POOL, METADATA_POOL, REFUSE_ALLOCATION,
    REFUSE_INVENTORY_WITH_ALLOCATOR, REFUSE_MATCHING_ALLOCATION_AFTER,
};
#[cfg(test)]
use crate::stylesheet_fetch::{StylesheetBodyEvidence, StylesheetFetchOutcome};
use crate::stylesheet_fetch::{
    StylesheetInventoryError as Error, StylesheetInventoryStorage as Storage,
};
use bus::css_decode::CssEncoding;
use runtime_css::decoding::{resolve_stylesheet_label, stylesheet_environment};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StylesheetPreparationFailure {
    Inventory(Error),
    ParserIntegrity {
        owner: html::internal::Id,
        slot: StylesheetSlotId,
    },
}
impl From<Error> for StylesheetPreparationFailure {
    fn from(e: Error) -> Self {
        Self::Inventory(e)
    }
}
#[cfg(test)]
thread_local! { pub(crate) static INJECT_INLINE_INTEGRITY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
fn parse_inline(
    text: &str,
    owner: html::internal::Id,
    slot: StylesheetSlotId,
) -> Result<StylesheetParse, StylesheetPreparationFailure> {
    #[allow(unused_mut)]
    let mut parse = parse_stylesheet_with_options(text, &ParseOptions::stylesheet());
    #[cfg(test)]
    INJECT_INLINE_INTEGRITY.with(|flag| {
        if flag.replace(false) {
            parse.stats.had_invariant_violation = true;
        }
    });
    if parse.completion() == css::StylesheetParseCompletion::InvariantViolation {
        Err(StylesheetPreparationFailure::ParserIntegrity { owner, slot })
    } else {
        Ok(parse)
    }
}
pub(crate) struct PreparedStylesheetReconcile {
    entries: Vec<PreparedSlot>,
    destination: HashMap<StylesheetSlotId, StylesheetSlot>,
    order: Vec<StylesheetSlotId>,
    projection: Vec<StylesheetParse>,
    retired: Vec<StylesheetSlotId>,
    dispatch: Vec<StylesheetSlotId>,
    next_id: u64,
    source_bytes: usize,
    metadata_bytes: usize,
    body_bytes: usize,
    changed: bool,
}
// Up to4096 records; reserve the Vec fallibly instead of boxing every new owner.
#[allow(clippy::large_enum_variant)]
enum PreparedSlot {
    Retained {
        id: StylesheetSlotId,
        media: PreparedMedia,
        host: HostEligibility,
        link_label: LinkLabel,
        state: PreparedRetainedState,
    },
    New {
        id: StylesheetSlotId,
        slot: StylesheetSlot,
    },
}
// Bounded prepared Vec owns large values inline; boxing adds an unqualified
// allocation per transition and weakens publication preparation.
#[allow(clippy::large_enum_variant)]
enum PreparedRetainedState {
    Keep,
    ParseInline(StylesheetParse),
    BeginExternal(Option<StylesheetFetchResult>),
}
enum PreparedMedia {
    Keep,
    Replace(Option<String>),
}
struct Frame<'a> {
    children: &'a [Node],
    next: usize,
}
fn children(node: &Node) -> &[Node] {
    match node {
        Node::Document { children, .. } => children,
        Node::Element { element } => element.children(),
        _ => &[],
    }
}
fn reserve<T>(v: &mut Vec<T>, count: usize, storage: Storage) -> Result<(), Error> {
    let required = v
        .len()
        .checked_add(count)
        .ok_or(Error::ArithmeticOverflow)?;
    if required > v.capacity() {
        let target = v
            .capacity()
            .max(4)
            .saturating_mul(2)
            .max(required)
            .min(ATTACHMENTS);
        crate::stylesheet_fetch::inventory_growth(storage)?;
        v.try_reserve_exact(target - v.len())
            .map_err(|_| Error::Reservation(storage))?;
    }
    Ok(())
}
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::ArithmeticOverflow)
}
fn charge(total: &mut usize, n: usize) -> Result<(), Error> {
    *total = add(*total, n)?;
    if *total > SOURCE_BYTES {
        Err(Error::SourceLimit)
    } else {
        Ok(())
    }
}
fn optional_copy(s: Option<&str>) -> Result<Option<String>, Error> {
    s.map(try_string).transpose()
}
fn text_length(children: &[Node]) -> Result<usize, Error> {
    children.iter().try_fold(0usize, |n, c| {
        add(
            n,
            match c {
                Node::Text { text, .. } => text.len(),
                _ => 0,
            },
        )
    })
}
fn text_equal(mut old: &str, children: &[Node]) -> bool {
    for child in children {
        if let Node::Text { text, .. } = child {
            let Some(rest) = old.strip_prefix(text.as_str()) else {
                return false;
            };
            old = rest;
        }
    }
    old.is_empty()
}
pub(super) fn source_charge(source: &StylesheetSlotSource) -> Result<usize, Error> {
    match source {
        StylesheetSlotSource::Inline(text) => Ok(text.len()),
        StylesheetSlotSource::External {
            href,
            address,
            diagnostics,
            url_failure,
            ..
        } => {
            let mut n = href.len();
            n = add(n, address.as_ref().map_or(0, |a| a.url().as_str().len()))?;
            n = add(
                n,
                diagnostics
                    .len()
                    .checked_mul(std::mem::size_of::<addressing::UrlDiagnostic>())
                    .ok_or(Error::ArithmeticOverflow)?,
            )?;
            if let Some(f) = url_failure {
                n = add(n, f.base.as_ref().map_or(0, |u| u.as_str().len()))?;
                n = add(
                    n,
                    f.diagnostics
                        .len()
                        .checked_mul(std::mem::size_of::<addressing::UrlDiagnostic>())
                        .ok_or(Error::ArithmeticOverflow)?,
                )?;
            }
            Ok(n)
        }
    }
}
impl DocumentStyleSet {
    #[cfg(test)]
    pub(crate) fn prepare_reconcile(
        &self,
        dom: &Node,
        base: Option<&BrowserUrl>,
        replaced: bool,
    ) -> Result<PreparedStylesheetReconcile, Error> {
        self.prepare_reconcile_with_environment(dom, base, replaced, None)
            .map_err(|e| match e {
                StylesheetPreparationFailure::Inventory(e) => e,
                StylesheetPreparationFailure::ParserIntegrity { .. } => {
                    panic!("fixture parser invariant")
                }
            })
    }
    pub(crate) fn prepare_reconcile_with_environment(
        &self,
        dom: &Node,
        base: Option<&BrowserUrl>,
        replaced: bool,
        document_encoding: Option<CssEncoding>,
    ) -> Result<PreparedStylesheetReconcile, StylesheetPreparationFailure> {
        let mut owners = HashMap::new();
        if !self.slots.is_empty() {
            crate::stylesheet_fetch::inventory_growth(Storage::Owners)?;
        }
        owners
            .try_reserve(self.slots.len())
            .map_err(|_| Error::Reservation(Storage::Owners))?;
        if !replaced {
            for (id, s) in self.ordered_slots() {
                if owners.insert(s.owner, id).is_some() {
                    return Err(Error::ArithmeticOverflow.into());
                }
            }
        }
        let mut p = PreparedStylesheetReconcile {
            entries: Vec::new(),
            destination: HashMap::new(),
            order: Vec::new(),
            projection: Vec::new(),
            retired: Vec::new(),
            dispatch: Vec::new(),
            next_id: self.next_slot_id,
            source_bytes: 0,
            metadata_bytes: 0,
            body_bytes: 0,
            changed: replaced,
        };
        let mut frames: Vec<Frame<'_>> = Vec::new();
        let mut next = Some(dom);
        let mut visited = 0usize;
        while let Some(node) = next.take() {
            if visited == ORDINARY_NODES {
                return Err(Error::OrdinaryNodeLimit.into());
            }
            visited += 1;
            let external = matches!(node,Node::Element{element}if element.namespace()==html::ElementNamespace::Html&&element.name()=="link")
                && node.attr_has_token("rel", "stylesheet");
            let inline = matches!(node,Node::Element{element}if element.namespace()==html::ElementNamespace::Html&&element.name()=="style");
            if external || inline {
                if p.entries.len() == ATTACHMENTS {
                    return Err(Error::AttachmentLimit.into());
                }
                let host = HostEligibility::from_node(node, external);
                let link_label = if external {
                    resolve_stylesheet_label(node.attr("charset"))
                } else {
                    LinkLabel::Absent
                };
                let environment = if external {
                    stylesheet_environment(link_label, document_encoding)
                } else {
                    None
                };
                let href = node.attr("href");
                let media = node.attr("media");
                let content = children(node);
                let old = owners.get(&node.id()).copied().filter(|id| {
                    match (&self.slots[id].source, external) {
                        (StylesheetSlotSource::Inline(text), false) => text_equal(text, content),
                        (
                            StylesheetSlotSource::External {
                                href: old,
                                href_present,
                                ..
                            },
                            true,
                        ) => {
                            *href_present == href.is_some()
                                && old == href.unwrap_or("")
                                && self.slots[id].environment == environment
                        }
                        _ => false,
                    }
                });
                reserve(&mut p.entries, 1, Storage::Entries)?;
                reserve(&mut p.order, 1, Storage::Order)?;
                if let Some(id) = old {
                    owners.remove(&node.id());
                    let slot = &self.slots[&id];
                    charge(&mut p.source_bytes, source_charge(&slot.source)?)?;
                    charge(&mut p.source_bytes, media.map_or(0, str::len))?;
                    if slot.metadata_reserved {
                        p.metadata_bytes = add(p.metadata_bytes, METADATA_BYTES)?;
                    }
                    if slot.body_reservation != BodyReservation::None {
                        p.body_bytes = add(p.body_bytes, BODY_BYTES)?;
                    }
                    let state = if !external
                        && host.parses_inline()
                        && matches!(slot.state, StylesheetSlotState::NotRequested)
                    {
                        let StylesheetSlotSource::Inline(text) = &slot.source else {
                            unreachable!()
                        };
                        PreparedRetainedState::ParseInline(parse_inline(text, node.id(), id)?)
                    } else if external
                        && host.participates()
                        && matches!(slot.state, StylesheetSlotState::NotRequested)
                    {
                        let failure = super::StylesheetLifecycleView { id, slot }.source_failure();
                        if failure.is_none() {
                            reserve(&mut p.dispatch, 1, Storage::Dispatch)?;
                            p.dispatch.push(id);
                        }
                        PreparedRetainedState::BeginExternal(failure.map(|f| {
                            StylesheetFetchResult::unissued(
                                StylesheetRequestEvidence::NotConstructed,
                                StylesheetFetchFailure::Source(f),
                            )
                        }))
                    } else {
                        PreparedRetainedState::Keep
                    };
                    let prospective = match &state {
                        PreparedRetainedState::ParseInline(p) => Some(p),
                        PreparedRetainedState::Keep if host.parses_inline() => match &slot.state {
                            StylesheetSlotState::Loaded(p) => Some(p),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(parse) = prospective.filter(|p| contributes(host, media, p)) {
                        reserve(&mut p.projection, 1, Storage::Projection)?;
                        p.projection.push(super::clone_for_projection(parse));
                    }
                    if self.order.get(p.order.len()) != Some(&id)
                        || slot.media.as_deref() != media
                        || slot.host != host
                        || !matches!(state, PreparedRetainedState::Keep)
                    {
                        p.changed = true;
                    }
                    p.entries.push(PreparedSlot::Retained {
                        id,
                        host,
                        link_label,
                        state,
                        media: if slot.media.as_deref() == media {
                            PreparedMedia::Keep
                        } else {
                            PreparedMedia::Replace(optional_copy(media)?)
                        },
                    });
                    p.order.push(id);
                } else {
                    p.changed = true;
                    p.next_id = p.next_id.checked_add(1).ok_or(Error::SlotIdExhausted)?;
                    let id = StylesheetSlotId(p.next_id);
                    let input_len = if external {
                        href.map_or(0, str::len)
                    } else {
                        text_length(content)?
                    };
                    charge(
                        &mut p.source_bytes,
                        add(input_len, media.map_or(0, str::len))?,
                    )?;
                    let mut failure = None;
                    let source = if external {
                        let raw = href.unwrap_or("");
                        let value = raw.trim();
                        let mut address = None;
                        let mut diagnostics = Vec::new();
                        let mut url_failure = None;
                        failure = if href.is_none() {
                            Some(StylesheetSourceFailure::MissingHref)
                        } else if raw.is_empty() {
                            Some(StylesheetSourceFailure::EmptyHref)
                        } else if value.is_empty() {
                            Some(StylesheetSourceFailure::EmptyAfterPreprocessing)
                        } else if base.is_none() {
                            Some(StylesheetSourceFailure::BaseUnavailable)
                        } else {
                            None
                        };
                        if failure.is_none() {
                            let base = base.expect("admitted base");
                            if add(value.len(), base.as_str().len())? > SOURCE_BYTES {
                                return Err(Error::SourceLimit.into());
                            }
                            match base.resolve(value) {
                                Ok(out) => {
                                    address = Some(ResourceAddress::new(
                                        out.url,
                                        ResourceKind::Stylesheet,
                                    ));
                                    diagnostics = out.diagnostics;
                                    let capability = address
                                        .as_ref()
                                        .expect("resolved")
                                        .capability(ResourceOperation::Retrieval);
                                    if capability != ResourceCapability::Supported {
                                        failure =
                                            Some(StylesheetSourceFailure::UnsupportedRetrieval(
                                                capability,
                                            ));
                                    }
                                }
                                Err(error) => {
                                    failure = Some(StylesheetSourceFailure::Url(error.kind));
                                    url_failure = Some(error);
                                }
                            }
                        }
                        let s = StylesheetSlotSource::External {
                            href: try_string(raw)?,
                            href_present: href.is_some(),
                            address,
                            diagnostics,
                            url_failure,
                            resource_base: None,
                        };
                        let extra = source_charge(&s)?
                            .checked_sub(raw.len())
                            .ok_or(Error::ArithmeticOverflow)?;
                        charge(&mut p.source_bytes, extra)?;
                        s
                    } else {
                        let mut text = String::new();
                        if input_len > 0 {
                            crate::stylesheet_fetch::inventory_growth(Storage::Source)?;
                        }
                        text.try_reserve_exact(input_len)
                            .map_err(|_| Error::Reservation(Storage::Source))?;
                        for c in content {
                            if let Node::Text { text: t, .. } = c {
                                text.push_str(t)
                            }
                        }
                        StylesheetSlotSource::Inline(text)
                    };
                    let state = match &source {
                        StylesheetSlotSource::Inline(text) if host.parses_inline() => {
                            StylesheetSlotState::Loaded(parse_inline(text, node.id(), id)?)
                        }
                        StylesheetSlotSource::Inline(_) => StylesheetSlotState::NotRequested,
                        _ if !host.participates() => StylesheetSlotState::NotRequested,
                        _ if failure.is_some() => StylesheetSlotState::Failed,
                        _ => StylesheetSlotState::Pending,
                    };
                    if let StylesheetSlotState::Loaded(parse) = &state
                        && contributes(host, media, parse)
                    {
                        reserve(&mut p.projection, 1, Storage::Projection)?;
                        p.projection.push(super::clone_for_projection(parse));
                    }
                    if host.participates() && failure.is_none() && external {
                        reserve(&mut p.dispatch, 1, Storage::Dispatch)?;
                        p.dispatch.push(id);
                    }
                    if !host.participates() {
                        failure = None;
                    }
                    let slot = StylesheetSlot {
                        owner: node.id(),
                        source,
                        media: optional_copy(media)?,
                        host,
                        link_label,
                        environment,
                        decoding: None,
                        runtime_receipt: None,
                        state,
                        fetch_result: failure.map(|f| {
                            StylesheetFetchResult::unissued(
                                StylesheetRequestEvidence::NotConstructed,
                                StylesheetFetchFailure::Source(f),
                            )
                        }),
                        installation_result: None,
                        metadata_reserved: false,
                        body_reservation: BodyReservation::None,
                    };
                    p.entries.push(PreparedSlot::New { id, slot });
                    p.order.push(id);
                }
            }
            let c = children(node);
            if !c.is_empty() {
                if frames.len() == frames.capacity() {
                    let target = frames.len().max(4).saturating_mul(2).min(ORDINARY_NODES);
                    crate::stylesheet_fetch::inventory_growth(Storage::Traversal)?;
                    frames
                        .try_reserve_exact(target - frames.len())
                        .map_err(|_| Error::Reservation(Storage::Traversal))?;
                }
                frames.push(Frame {
                    children: c,
                    next: 0,
                });
            }
            while let Some(frame) = frames.last_mut() {
                if let Some(child) = frame.children.get(frame.next) {
                    frame.next += 1;
                    next = Some(child);
                    break;
                } else {
                    frames.pop();
                }
            }
        }
        if p.order.len() != self.order.len() {
            p.changed = true;
        }
        if !p.order.is_empty() {
            crate::stylesheet_fetch::inventory_growth(Storage::Destination)?;
        }
        p.destination
            .try_reserve(p.order.len())
            .map_err(|_| Error::Reservation(Storage::Destination))?;
        if p.order.len() > p.projection.capacity() {
            crate::stylesheet_fetch::inventory_growth(Storage::Projection)?;
        }
        p.projection
            .try_reserve_exact(p.order.len().saturating_sub(p.projection.len()))
            .map_err(|_| Error::Reservation(Storage::Projection))?;
        reserve(&mut p.retired, self.order.len(), Storage::Retirement)?;
        for (id, slot) in self.ordered_slots() {
            if replaced || owners.get(&slot.owner) == Some(&id) {
                p.retired.push(id);
            }
        }
        Ok(p)
    }
    pub(crate) fn commit_reconcile(
        &mut self,
        mut p: PreparedStylesheetReconcile,
    ) -> StylesheetReconcileResult {
        let mut old = std::mem::take(&mut self.slots);
        for entry in p.entries {
            let (id, slot) = match entry {
                PreparedSlot::Retained {
                    id,
                    media,
                    host,
                    link_label,
                    state,
                } => {
                    let mut slot = old.remove(&id).expect("prepared retained owner");
                    if let PreparedMedia::Replace(media) = media {
                        slot.media = media;
                    }
                    slot.host = host;
                    slot.link_label = link_label;
                    match state {
                        PreparedRetainedState::Keep => {}
                        PreparedRetainedState::ParseInline(parse) => {
                            slot.state = StylesheetSlotState::Loaded(parse)
                        }
                        PreparedRetainedState::BeginExternal(result) => {
                            slot.state = if result.is_some() {
                                StylesheetSlotState::Failed
                            } else {
                                StylesheetSlotState::Pending
                            };
                            slot.fetch_result = result;
                        }
                    }
                    (id, slot)
                }
                PreparedSlot::New { id, slot } => (id, slot),
            };
            p.destination.insert(id, slot);
        }
        self.slots = p.destination;
        self.order = p.order;
        self.loaded_stylesheets = p.projection;
        self.next_slot_id = p.next_id;
        self.source_bytes = p.source_bytes;
        self.metadata_bytes = p.metadata_bytes;
        self.body_bytes = p.body_bytes;
        StylesheetReconcileResult {
            fetches: p.dispatch,
            retired: p.retired,
            changed: p.changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dom(s: &str) -> html::ParseOutput {
        let mut options = html::HtmlParseOptions::default();
        options.tokenizer.limits.max_attribute_value_bytes = SOURCE_BYTES + 1;
        let mut out = html::parse_document(s, options).unwrap();
        html::internal::assign_missing_full_model_ids_for_test(&mut out.document);
        out
    }
    #[test]
    fn aj7_inventory_count_source_and_traversal_exact_boundaries() {
        let source = "<link rel=stylesheet>".repeat(ATTACHMENTS);
        let output = dom(&source);
        let set = DocumentStyleSet::default();
        assert!(set.prepare_reconcile(&output.document, None, false).is_ok());
        let output = dom(&(source + "<style></style>"));
        assert!(matches!(
            set.prepare_reconcile(&output.document, None, false),
            Err(Error::AttachmentLimit)
        ));
        let source = format!("<link rel=stylesheet href='{}'>", "x".repeat(SOURCE_BYTES));
        let output = dom(&source);
        let mut set = DocumentStyleSet::default();
        let prepared = set
            .prepare_reconcile(&output.document, None, false)
            .unwrap();
        set.commit_reconcile(prepared);
        assert_eq!(set.source_bytes, SOURCE_BYTES);
        let before = set.slot_count();
        let output = dom(&format!(
            "<link rel=stylesheet href='{}'>",
            "x".repeat(SOURCE_BYTES + 1)
        ));
        assert!(matches!(
            set.prepare_reconcile(&output.document, None, false),
            Err(Error::SourceLimit)
        ));
        assert_eq!(set.slot_count(), before);
        let mut tree = Node::Document {
            id: Id(1),
            doctype: None,
            children: (0..ORDINARY_NODES - 1)
                .map(|i| Node::Text {
                    id: Id(i as u32 + 2),
                    text: String::new(),
                })
                .collect(),
        };
        assert!(set.prepare_reconcile(&tree, None, false).is_ok());
        if let Node::Document { children, .. } = &mut tree {
            children.push(Node::Text {
                id: Id(999999),
                text: String::new(),
            });
        }
        assert!(matches!(
            set.prepare_reconcile(&tree, None, false),
            Err(Error::OrdinaryNodeLimit)
        ));
    }
    #[test]
    fn aj7_prepare_retains_frozen_source_and_moves_terminal_bytes_without_copy() {
        let output = dom("<link rel=stylesheet href=s.css>");
        let base = BrowserUrl::parse("https://one.test/").unwrap().url;
        let next_base = BrowserUrl::parse("https://two.test/").unwrap().url;
        let mut set = DocumentStyleSet::default();
        let prepared = set
            .prepare_reconcile(&output.document, Some(&base), false)
            .unwrap();
        let first = set.commit_reconcile(prepared);
        let id = first.fetches[0];
        set.reserve_metadata(id).unwrap();
        set.provision_body(id).unwrap();
        set.issue_body(id);
        let bytes = vec![42; 3000];
        let pointer = bytes.as_ptr();
        let mut result = StylesheetFetchResult::unissued(
            StylesheetRequestEvidence::NotConstructed,
            StylesheetFetchFailure::DocumentAborted,
        );
        result.body = StylesheetBodyEvidence::Complete(bytes);
        set.finish_fetch(id, result);
        let p = set
            .prepare_reconcile(&output.document, Some(&next_base), false)
            .unwrap();
        assert_eq!(
            set.fetch_result(id).unwrap().body.bytes().unwrap().as_ptr(),
            pointer
        );
        assert!(p.dispatch.is_empty());
        set.commit_reconcile(p);
        assert_eq!(
            set.selected_address(id).unwrap().url().as_str(),
            "https://one.test/s.css"
        );
        assert_eq!(
            set.fetch_result(id).unwrap().body.bytes().unwrap().as_ptr(),
            pointer
        );
        assert_eq!(set.retention_usage().1, METADATA_BYTES);
        assert_eq!(set.retention_usage().2, BODY_BYTES);
        let p = set
            .prepare_reconcile(&output.document, Some(&next_base), true)
            .unwrap();
        let next = set.commit_reconcile(p);
        assert_ne!(next.fetches[0], id);
        assert_eq!(next.retired, vec![id]);
        assert_eq!(set.retention_usage().1, 0);
        assert_eq!(set.retention_usage().2, 0);
    }
    #[test]
    fn aj7_preparation_dispatch_is_id_only_and_unchanged_media_keeps_its_owner() {
        let output = dom(
            "<link rel=stylesheet href=one.css media=screen><link rel=stylesheet href=two.css>",
        );
        let base = BrowserUrl::parse("https://one.test/").unwrap().url;
        let mut set = DocumentStyleSet::default();
        let p = set
            .prepare_reconcile(&output.document, Some(&base), false)
            .unwrap();
        let first = set.commit_reconcile(p);
        assert_eq!(first.fetches, set.order);
        let id = first.fetches[0];
        let media = set.slots[&id].media.as_ref().unwrap().as_ptr();
        let address = set.selected_address(id).unwrap().url().as_str().as_ptr();
        let p = set
            .prepare_reconcile(&output.document, Some(&base), false)
            .unwrap();
        assert!(matches!(
            &p.entries[0],
            PreparedSlot::Retained {
                media: PreparedMedia::Keep,
                ..
            }
        ));
        assert!(p.dispatch.is_empty());
        let (_, allocations) =
            crate::tab::tests::parsed_document_observation::allocation_work(|| {
                set.commit_reconcile(p)
            });
        assert_eq!(allocations, [0, 0, 0]);
        assert_eq!(set.slots[&id].media.as_ref().unwrap().as_ptr(), media);
        assert_eq!(
            set.selected_address(id).unwrap().url().as_str().as_ptr(),
            address
        );
        let changed =
            dom("<link rel=stylesheet href=one.css media=print><link rel=stylesheet href=two.css>")
                .document;
        let p = set.prepare_reconcile(&changed, Some(&base), false).unwrap();
        assert!(
            matches!(&p.entries[0],PreparedSlot::Retained{media:PreparedMedia::Replace(Some(value)),..}if value=="print")
        );
        assert!(p.dispatch.is_empty());
        set.commit_reconcile(p);
        assert_eq!(set.slots[&id].media.as_deref(), Some("print"));
    }
    #[test]
    fn aj7_each_prepared_storage_refusal_preserves_old_owners_and_ledgers() {
        for actual_allocator in [false, true] {
            REFUSE_INVENTORY_WITH_ALLOCATOR.with(|mode| mode.set(actual_allocator));
            for storage in [
                Storage::Traversal,
                Storage::Owners,
                Storage::Entries,
                Storage::Source,
                Storage::Order,
                Storage::Destination,
                Storage::Projection,
                Storage::Retirement,
                Storage::Dispatch,
            ] {
                let old = dom("<link rel=stylesheet href=old.css media=screen>");
                let new = dom("<link rel=stylesheet href=new.css media=print>");
                let base = BrowserUrl::parse("https://one.test/").unwrap().url;
                let mut set = DocumentStyleSet::default();
                let p = set
                    .prepare_reconcile(&old.document, Some(&base), false)
                    .unwrap();
                let id = set.commit_reconcile(p).fetches[0];
                set.reserve_metadata(id).unwrap();
                set.provision_body(id).unwrap();
                set.issue_body(id);
                let mut result = StylesheetFetchResult::unissued(
                    StylesheetRequestEvidence::NotConstructed,
                    StylesheetFetchFailure::DocumentAborted,
                );
                result.body = StylesheetBodyEvidence::Complete(vec![42; 3000]);
                let pointer = result.body.bytes().unwrap().as_ptr();
                set.finish_fetch(id, result);
                let usage = set.retention_usage();
                let next = set.next_slot_id;
                let media = set.slots[&id].media.as_ref().unwrap().as_ptr();
                REFUSE_ALLOCATION
                    .with(|refusal| refusal.set(Some(AllocationBoundary::Inventory(storage))));
                assert!(
                    matches!(set.prepare_reconcile(&new.document,Some(&base),false),Err(Error::Reservation(actual))if actual==storage),
                    "{storage:?}"
                );
                assert_eq!(REFUSE_ALLOCATION.with(|refusal| refusal.get()), None);
                assert_eq!(set.order, vec![id]);
                assert_eq!(set.retention_usage(), usage);
                assert_eq!(set.next_slot_id, next);
                assert_eq!(
                    set.fetch_result(id).unwrap().body.bytes().unwrap().as_ptr(),
                    pointer
                );
                assert_eq!(set.slots[&id].media.as_ref().unwrap().as_ptr(), media);
                assert_eq!(
                    set.selected_address(id).unwrap().url().as_str(),
                    "https://one.test/old.css"
                );
                assert!(
                    crate::tab::tests::parsed_document_observation::inventory_refusal_consumed()
                );
            }
            REFUSE_INVENTORY_WITH_ALLOCATOR.with(|mode| mode.set(false));
        }
    }
    #[test]
    fn aj7_later_source_and_final_projection_reservations_refuse_atomically() {
        for actual_allocator in [false, true] {
            REFUSE_INVENTORY_WITH_ALLOCATOR.with(|mode| mode.set(actual_allocator));
            for (markup, storage) in [
                (
                    "<style media=screen>p{color:red}</style>".to_owned(),
                    Storage::Source,
                ),
                (
                    "<link rel=stylesheet href=new.css media=screen>".to_owned(),
                    Storage::Source,
                ),
                (
                    "<link rel=stylesheet href=new.css>".repeat(9),
                    Storage::Projection,
                ),
            ] {
                let tree = dom(&markup);
                let set = DocumentStyleSet::default();
                let base = BrowserUrl::parse("https://example.test/").unwrap().url;
                REFUSE_ALLOCATION
                    .with(|point| point.set(Some(AllocationBoundary::Inventory(storage))));
                REFUSE_MATCHING_ALLOCATION_AFTER
                    .with(|remaining| remaining.set(usize::from(storage != Storage::Projection)));
                assert!(
                    matches!(set.prepare_reconcile(&tree.document,Some(&base),false),Err(Error::Reservation(actual))if actual==storage)
                );
                assert_eq!(set.slot_count(), 0);
                assert_eq!(set.retention_usage(), (0, 0, 0));
                assert_eq!(REFUSE_ALLOCATION.with(|point| point.get()), None);
                assert_eq!(
                    REFUSE_MATCHING_ALLOCATION_AFTER.with(|remaining| remaining.get()),
                    0
                );
                assert!(
                    crate::tab::tests::parsed_document_observation::inventory_refusal_consumed()
                );
            }
            REFUSE_INVENTORY_WITH_ALLOCATOR.with(|mode| mode.set(false));
        }
    }
    #[test]
    fn aj7_deep_iterative_inventory_and_source_base_unavailability() {
        let mut tree = dom("<link rel=stylesheet href=relative.css>").document;
        for i in 0..1024 {
            tree = Node::Document {
                id: Id(100000 + i),
                doctype: None,
                children: vec![tree],
            };
        }
        let mut set = DocumentStyleSet::default();
        let prepared = set.prepare_reconcile(&tree, None, false).unwrap();
        let result = set.commit_reconcile(prepared);
        assert!(result.fetches.is_empty());
        let slot = set.lifecycle_slots().next().unwrap();
        assert_eq!(
            slot.source_failure(),
            Some(StylesheetSourceFailure::BaseUnavailable)
        );
        assert_eq!(
            slot.fetch_result().unwrap().outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(
                StylesheetSourceFailure::BaseUnavailable
            ))
        );
        // Explicit iterative teardown does not make DOM destruction part of this
        // traversal qualification.
        let mut stack = vec![tree];
        while let Some(mut node) = stack.pop() {
            if let Some(children) = node.children_mut() {
                stack.append(children);
            }
        }
    }
    #[test]
    fn aj7_independent_ledger_exact_limit_rollback_and_retirement() {
        let output = dom(&"<link rel=stylesheet href=s.css>".repeat(33));
        let base = BrowserUrl::parse("https://one.test/").unwrap().url;
        let mut set = DocumentStyleSet::default();
        let p = set
            .prepare_reconcile(&output.document, Some(&base), false)
            .unwrap();
        let slots = set.commit_reconcile(p).fetches;
        for s in &slots[..32] {
            set.reserve_metadata(*s).unwrap();
            set.provision_body(*s).unwrap();
            set.rollback_body(*s);
        }
        assert_eq!(set.body_bytes, 0);
        assert_eq!(set.metadata_bytes, METADATA_POOL);
        assert_eq!(
            set.reserve_metadata(slots[32]),
            Err(StylesheetRetentionFailure::MetadataReservationDenied)
        );
        for s in &slots[..32] {
            set.provision_body(*s).unwrap();
            set.issue_body(*s);
        }
        assert_eq!(set.body_bytes, BODY_POOL);
        assert_eq!(
            set.provision_body(slots[32]),
            Err(StylesheetRetentionFailure::BodyReservationDenied)
        );
        let empty = dom("");
        let p = set
            .prepare_reconcile(&empty.document, Some(&base), false)
            .unwrap();
        set.commit_reconcile(p);
        assert_eq!(set.retention_usage(), (0, 0, 0));
    }
}
#[cfg(test)]
mod id_exhaustion_test {
    use super::*;
    #[test]
    fn aj7_slot_identity_exhaustion_preserves_old_inventory() {
        let set = DocumentStyleSet {
            next_slot_id: u64::MAX,
            ..Default::default()
        };
        let output =
            html::parse_document("<link rel=stylesheet>", html::HtmlParseOptions::default())
                .unwrap();
        assert!(matches!(
            set.prepare_reconcile(&output.document, None, false),
            Err(Error::SlotIdExhausted)
        ));
        assert_eq!(set.next_slot_id, u64::MAX);
        assert_eq!(set.slot_count(), 0);
        assert_eq!(set.retention_usage(), (0, 0, 0));
    }
}

#[cfg(test)]
mod aj8_guards {
    use super::*;
    #[test]
    fn aj8_sequential_projection_clone_and_prepared_commit_bounds() {
        for (n, expected) in [(4, 10), (16, 136), (32, 528)] {
            let mut output = html::parse_document(
                format!(
                    "<!doctype html>{}",
                    "<link rel=stylesheet href=a.css>".repeat(n)
                ),
                html::HtmlParseOptions::default(),
            )
            .unwrap();
            html::internal::assign_missing_full_model_ids_for_test(&mut output.document);
            let base = BrowserUrl::parse("https://example.test/").unwrap().url;
            let mut set = DocumentStyleSet::default();
            let ids = set
                .reconcile_from_dom(&output.document, Some(&base))
                .fetches;
            super::super::PROJECTION_CLONES.with(|c| c.set(0));
            for id in ids {
                assert!(set.install_external_stylesheet(id, "p{}", &super::super::test_response()));
            }
            assert_eq!(super::super::PROJECTION_CLONES.with(|c| c.get()), expected);
            super::super::PROJECTION_CLONES.with(|c| c.set(0));
            let prepared = set
                .prepare_reconcile(&output.document, Some(&base), false)
                .unwrap();
            assert_eq!(super::super::PROJECTION_CLONES.with(|c| c.get()), n);
            // At this point authoritative + old compatibility + prepared projection
            // each owns N model graphs, including deep-cloned CssInput buffers.
            assert_eq!(
                set.slots.len() + set.loaded_stylesheets.len() + prepared.projection.len(),
                3 * n
            );
            let (_, counts) =
                crate::tab::tests::parsed_document_observation::allocation_work(|| {
                    set.commit_reconcile(prepared)
                });
            assert_eq!(counts, [0, 0, 0]);
            let mut replacement = html::parse_document(
                format!(
                    "<!doctype html>{}",
                    "<style>p{color:blue}</style>".repeat(n)
                ),
                html::HtmlParseOptions::default(),
            )
            .unwrap();
            html::internal::assign_missing_full_model_ids_for_test(&mut replacement.document);
            let prepared = set
                .prepare_reconcile(&replacement.document, Some(&base), true)
                .unwrap();
            let new_parses = prepared
                .entries
                .iter()
                .filter(|entry| {
                    matches!(
                        entry,
                        PreparedSlot::New {
                            slot: StylesheetSlot {
                                state: StylesheetSlotState::Loaded(_),
                                ..
                            },
                            ..
                        }
                    )
                })
                .count();
            assert_eq!(new_parses, n);
            assert_eq!(
                set.slots.len()
                    + set.loaded_stylesheets.len()
                    + new_parses
                    + prepared.projection.len(),
                4 * n
            );
            println!(
                "AJ8 clone workload N={n} sequential={expected} retained_prepare_graphs={} commit_allocations=0",
                3 * n
            );
        }
    }
}
