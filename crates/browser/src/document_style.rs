use crate::stylesheet_fetch::{
    BODY_BYTES, BODY_POOL, METADATA_BYTES, METADATA_POOL, StylesheetFetchResult,
    StylesheetInstallationFailure, StylesheetRetentionFailure, StylesheetSourceFailure,
};
use addressing::{
    BrowserUrl, ResourceAddress, ResourceCapability, ResourceKind, ResourceOperation, UrlFailure,
    UrlParseOutcome,
};
use core_types::StylesheetSlotId;
use css::{
    ParseOptions, StylesheetCollectionInput, StylesheetCollectionInputBuildError,
    StylesheetConditionInput, StylesheetOrder, StylesheetParse, StylesheetSourceId,
    parse_stylesheet_with_options,
};
use html::Node;
use html::internal::Id;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;
use std::sync::OnceLock;
mod participation;
mod reconcile;
pub use participation::{
    CssRuntimeReceipt, HostEligibility, InlineType, SheetTitle, StylesheetDecodingOutcome,
};
use participation::{Environment, LinkLabel, complete, contributes};
#[cfg(test)]
pub(crate) use reconcile::INJECT_INLINE_INTEGRITY;
pub(crate) use reconcile::{PreparedStylesheetReconcile, StylesheetPreparationFailure};

#[cfg(test)]
thread_local! { static PROJECTION_CLONES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
fn clone_for_projection(parse: &StylesheetParse) -> StylesheetParse {
    #[cfg(test)]
    PROJECTION_CLONES.with(|n| n.set(n.get() + 1));
    parse.clone()
}
const MINIMAL_UA_STYLESHEET: &str = r#"
html, body, div, p, section, article, header, footer, main, nav, aside,
h1, h2, h3, h4, h5, h6, ul, ol, menu, form, table, thead, tbody, tfoot,
tr, td, th, blockquote, pre, address, figure, figcaption, dl, dt, dd {
    display: block;
}

li {
    display: list-item;
}

input, button, textarea {
    display: inline-block;
}

head, title, meta, link, style, script {
    display: none;
}
"#;

// Inline storage avoids an extra infallible allocation per qualified owner.
#[allow(clippy::large_enum_variant)]
enum StylesheetSlotSource {
    Inline(String),
    External {
        href: String,
        href_present: bool,
        diagnostics: Vec<addressing::UrlDiagnostic>,
        url_failure: Option<UrlFailure>,
        address: Option<ResourceAddress>,
        resource_base: Option<Box<Result<UrlParseOutcome, UrlFailure>>>,
    },
}

impl std::fmt::Debug for StylesheetSlotSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inline(_) => f.debug_tuple("Inline").field(&"<redacted>").finish(),
            Self::External {
                href,
                address,
                resource_base,
                ..
            } => f
                .debug_struct("External")
                .field("href", &if href.is_empty() { "" } else { "<redacted>" })
                .field("address", address)
                .field("resource_base", resource_base)
                .finish(),
        }
    }
}

pub(crate) enum StylesheetResourceBase<'a> {
    Document,
    Response(Option<&'a Result<UrlParseOutcome, UrlFailure>>),
}
pub(crate) struct StylesheetResourceSource<'a> {
    pub(crate) slot: StylesheetSlotId,
    pub(crate) owner: Id,
    pub(crate) parse: &'a StylesheetParse,
    pub(crate) media: Option<&'a str>,
    pub(crate) base: StylesheetResourceBase<'a>,
}

#[derive(Clone, Debug)]
enum StylesheetSlotState {
    NotRequested,
    Pending,
    Loaded(StylesheetParse),
    Failed,
    Aborted,
}

struct StylesheetSlot {
    owner: Id,
    source: StylesheetSlotSource,
    media: Option<String>,
    host: HostEligibility,
    link_label: LinkLabel,
    environment: Environment,
    decoding: Option<StylesheetDecodingOutcome>,
    runtime_receipt: Option<CssRuntimeReceipt>,
    state: StylesheetSlotState,
    fetch_result: Option<StylesheetFetchResult>,
    installation_result: Option<Result<(), StylesheetInstallationFailure>>,
    metadata_reserved: bool,
    body_reservation: BodyReservation,
}

/// A call-local borrow of an authoritative slot. No parse/text or mutation API
/// escapes this module; lifecycle consumers only inspect existing facts.
#[cfg(any(test, feature = "conformance-observation"))]
pub struct StylesheetSourceObservation<'a> {
    href: Option<&'a str>,
    media: Option<&'a str>,
    address: Option<&'a ResourceAddress>,
    diagnostics: &'a [addressing::UrlDiagnostic],
    url_failure: Option<&'a UrlFailure>,
    reason: Option<StylesheetSourceFailure>,
}
#[cfg(any(test, feature = "conformance-observation"))]
impl<'a> StylesheetSourceObservation<'a> {
    pub fn authored_href(&self) -> Option<&'a str> {
        self.href
    }
    pub fn media(&self) -> Option<&'a str> {
        self.media
    }
    pub fn address(&self) -> Option<&'a ResourceAddress> {
        self.address
    }
    pub fn diagnostics(&self) -> &'a [addressing::UrlDiagnostic] {
        self.diagnostics
    }
    pub fn url_failure(&self) -> Option<&'a UrlFailure> {
        self.url_failure
    }
    pub fn failure_reason(&self) -> Option<StylesheetSourceFailure> {
        self.reason
    }
}
#[derive(Clone, Copy)]
pub(crate) struct StylesheetLifecycleView<'a> {
    id: StylesheetSlotId,
    slot: &'a StylesheetSlot,
}

impl<'a> StylesheetLifecycleView<'a> {
    pub(crate) fn host(&self) -> HostEligibility {
        self.slot.host
    }
    pub(crate) fn media(&self) -> Option<&'a str> {
        self.slot.media.as_deref()
    }
    pub(crate) fn parse(&self) -> Option<&'a StylesheetParse> {
        match &self.slot.state {
            StylesheetSlotState::Loaded(p) => Some(p),
            _ => None,
        }
    }
    pub(crate) fn decoding(&self) -> Option<StylesheetDecodingOutcome> {
        self.slot.decoding
    }
    pub(crate) fn runtime_receipt(&self) -> Option<CssRuntimeReceipt> {
        self.slot.runtime_receipt
    }
    pub(crate) fn is_not_requested(&self) -> bool {
        matches!(self.slot.state, StylesheetSlotState::NotRequested)
    }
    pub(crate) fn slot_id(&self) -> StylesheetSlotId {
        self.id
    }
    pub(crate) fn is_external(&self) -> bool {
        matches!(&self.slot.source, StylesheetSlotSource::External { .. })
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub(crate) fn source_observation(&self) -> Option<StylesheetSourceObservation<'a>> {
        let StylesheetSlotSource::External {
            href,
            href_present,
            address,
            diagnostics,
            url_failure,
            ..
        } = &self.slot.source
        else {
            return None;
        };
        Some(StylesheetSourceObservation {
            href: href_present.then_some(href.as_str()),
            media: self.slot.media.as_deref(),
            address: address.as_ref(),
            diagnostics: url_failure
                .as_ref()
                .map_or(diagnostics.as_slice(), |failure| {
                    failure.diagnostics.as_slice()
                }),
            url_failure: url_failure.as_ref(),
            reason: self.source_failure(),
        })
    }
    pub(crate) fn selected_address(&self) -> Option<&'a ResourceAddress> {
        match &self.slot.source {
            StylesheetSlotSource::External { address, .. } => address.as_ref(),
            StylesheetSlotSource::Inline(_) => None,
        }
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub(crate) fn owner(&self) -> Id {
        self.slot.owner
    }
    pub(crate) fn fetch_result(&self) -> Option<&'a StylesheetFetchResult> {
        self.slot.fetch_result.as_ref()
    }
    pub(crate) fn installation_result(&self) -> Option<Result<(), StylesheetInstallationFailure>> {
        self.slot.installation_result
    }
    pub(crate) fn metadata_reserved(&self) -> bool {
        self.slot.metadata_reserved
    }
    pub(crate) fn body_provisional(&self) -> bool {
        self.slot.body_reservation == BodyReservation::Provisional
    }
    pub(crate) fn source_payload_charge(&self) -> Option<usize> {
        reconcile::source_charge(&self.slot.source)
            .ok()?
            .checked_add(self.slot.media.as_ref().map_or(0, String::len))
    }
    pub(crate) fn resource_base(&self) -> Option<&'a Result<UrlParseOutcome, UrlFailure>> {
        match &self.slot.source {
            StylesheetSlotSource::External { resource_base, .. } => resource_base.as_deref(),
            _ => None,
        }
    }
    pub(crate) fn source_failure(&self) -> Option<StylesheetSourceFailure> {
        match &self.slot.source {
            StylesheetSlotSource::Inline(_) => None,
            StylesheetSlotSource::External {
                href,
                href_present,
                address,
                url_failure,
                ..
            } => {
                if !href_present {
                    Some(StylesheetSourceFailure::MissingHref)
                } else if href.is_empty() {
                    Some(StylesheetSourceFailure::EmptyHref)
                } else if href.trim().is_empty() {
                    Some(StylesheetSourceFailure::EmptyAfterPreprocessing)
                } else if let Some(failure) = url_failure {
                    Some(StylesheetSourceFailure::Url(failure.kind))
                } else if let Some(address) = address {
                    let capability = address.capability(ResourceOperation::Retrieval);
                    (capability != ResourceCapability::Supported)
                        .then_some(StylesheetSourceFailure::UnsupportedRetrieval(capability))
                } else {
                    Some(StylesheetSourceFailure::BaseUnavailable)
                }
            }
        }
    }
    pub(crate) fn body_issued(&self) -> bool {
        self.slot.body_reservation == BodyReservation::Issued
    }
    pub(crate) fn is_pending(&self) -> bool {
        matches!(&self.slot.state, StylesheetSlotState::Pending)
    }
    pub(crate) fn has_loaded_parse(&self) -> bool {
        matches!(&self.slot.state, StylesheetSlotState::Loaded(_))
    }
    pub(crate) fn is_failed(&self) -> bool {
        matches!(&self.slot.state, StylesheetSlotState::Failed)
    }
    pub(crate) fn is_aborted(&self) -> bool {
        matches!(&self.slot.state, StylesheetSlotState::Aborted)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StylesheetReconcileResult {
    pub(crate) fetches: Vec<StylesheetSlotId>,
    pub(crate) retired: Vec<StylesheetSlotId>,
    pub(crate) changed: bool,
}

#[derive(Default)]
pub(crate) struct DocumentStyleSet {
    next_slot_id: u64,
    // Records have one owner. The ID sequence preserves document/cascade order
    // independently of point lookup and monotonic request identity.
    slots: HashMap<StylesheetSlotId, StylesheetSlot>,
    order: Vec<StylesheetSlotId>,
    loaded_stylesheets: Vec<StylesheetParse>,
    source_bytes: usize,
    metadata_bytes: usize,
    body_bytes: usize,
    #[cfg(test)]
    slot_lookups: std::cell::Cell<usize>,
}

#[derive(Debug, PartialEq, Eq)]
enum BodyReservation {
    None,
    Provisional,
    Issued,
}

impl DocumentStyleSet {
    pub(crate) fn slot_id_at(&self, ordinal: usize) -> Option<StylesheetSlotId> {
        self.order.get(ordinal).copied()
    }
    pub(crate) fn slot_count(&self) -> usize {
        self.order.len()
    }
    pub(crate) fn environment(&self, id: StylesheetSlotId) -> Environment {
        self.slots[&id].environment
    }
    pub(crate) fn decoding(&self, id: StylesheetSlotId) -> Option<StylesheetDecodingOutcome> {
        self.slots.get(&id)?.decoding
    }
    pub(crate) fn set_decoding(
        &mut self,
        id: StylesheetSlotId,
        outcome: StylesheetDecodingOutcome,
    ) {
        self.slots
            .get_mut(&id)
            .expect("current attachment")
            .decoding = Some(outcome);
    }
    pub(crate) fn set_runtime_receipt(&mut self, id: StylesheetSlotId, receipt: CssRuntimeReceipt) {
        let slot = self.slots.get_mut(&id).expect("current attachment");
        assert!(slot.runtime_receipt.is_none());
        slot.runtime_receipt = Some(receipt);
    }
    pub(crate) fn selected_address(&self, id: StylesheetSlotId) -> Option<&ResourceAddress> {
        match &self.slots.get(&id)?.source {
            StylesheetSlotSource::External { address, .. } => address.as_ref(),
            _ => None,
        }
    }
    #[cfg(test)]
    pub(crate) fn fetch_mut_for_tests(
        &mut self,
        id: StylesheetSlotId,
    ) -> Option<&mut StylesheetFetchResult> {
        self.slots.get_mut(&id)?.fetch_result.as_mut()
    }
    #[cfg(any(test, feature = "conformance-observation"))]
    pub(crate) fn source_observation(
        &self,
        id: StylesheetSlotId,
    ) -> Option<StylesheetSourceObservation<'_>> {
        StylesheetLifecycleView {
            id,
            slot: self.slots.get(&id)?,
        }
        .source_observation()
    }
    pub(crate) fn fetch_result(&self, id: StylesheetSlotId) -> Option<&StylesheetFetchResult> {
        self.slots.get(&id)?.fetch_result.as_ref()
    }
    pub(crate) fn installation_result(
        &self,
        id: StylesheetSlotId,
    ) -> Option<Result<(), StylesheetInstallationFailure>> {
        self.slots.get(&id)?.installation_result
    }
    pub(crate) fn retention_usage(&self) -> (usize, usize, usize) {
        (self.source_bytes, self.metadata_bytes, self.body_bytes)
    }
    pub(crate) fn body_available(&self) -> bool {
        self.body_bytes
            .checked_add(BODY_BYTES)
            .is_some_and(|n| n <= BODY_POOL)
    }
    pub(crate) fn reserve_metadata(
        &mut self,
        id: StylesheetSlotId,
    ) -> Result<(), StylesheetRetentionFailure> {
        let n = self
            .metadata_bytes
            .checked_add(METADATA_BYTES)
            .filter(|n| *n <= METADATA_POOL)
            .ok_or(StylesheetRetentionFailure::MetadataReservationDenied)?;
        let slot = self.slots.get_mut(&id).expect("current attachment");
        assert!(!slot.metadata_reserved);
        slot.metadata_reserved = true;
        self.metadata_bytes = n;
        Ok(())
    }
    pub(crate) fn release_unused_metadata(&mut self, id: StylesheetSlotId) {
        let slot = self.slots.get_mut(&id).expect("current attachment");
        if slot.metadata_reserved {
            slot.metadata_reserved = false;
            self.metadata_bytes = self
                .metadata_bytes
                .checked_sub(METADATA_BYTES)
                .expect("metadata accounting");
        }
    }
    pub(crate) fn provision_body(
        &mut self,
        id: StylesheetSlotId,
    ) -> Result<(), StylesheetRetentionFailure> {
        let n = self
            .body_bytes
            .checked_add(BODY_BYTES)
            .filter(|n| *n <= BODY_POOL)
            .ok_or(StylesheetRetentionFailure::BodyReservationDenied)?;
        let slot = self.slots.get_mut(&id).expect("current attachment");
        assert_eq!(slot.body_reservation, BodyReservation::None);
        slot.body_reservation = BodyReservation::Provisional;
        self.body_bytes = n;
        Ok(())
    }
    pub(crate) fn issue_body(&mut self, id: StylesheetSlotId) {
        let slot = self.slots.get_mut(&id).expect("current attachment");
        assert_eq!(slot.body_reservation, BodyReservation::Provisional);
        slot.body_reservation = BodyReservation::Issued;
    }
    pub(crate) fn rollback_body(&mut self, id: StylesheetSlotId) {
        let slot = self.slots.get_mut(&id).expect("current attachment");
        if slot.body_reservation == BodyReservation::Provisional {
            slot.body_reservation = BodyReservation::None;
            self.body_bytes = self
                .body_bytes
                .checked_sub(BODY_BYTES)
                .expect("body accounting");
        }
    }
    pub(crate) fn finish_fetch(&mut self, id: StylesheetSlotId, result: StylesheetFetchResult) {
        let slot = self.slots.get_mut(&id).expect("current attachment");
        assert!(slot.fetch_result.is_none());
        slot.fetch_result = Some(result);
    }
    #[cfg(test)]
    pub(crate) fn take_fetch_for_tests(
        &mut self,
        id: StylesheetSlotId,
    ) -> Option<StylesheetFetchResult> {
        self.slots.get_mut(&id)?.fetch_result.take()
    }
    #[cfg(test)]
    pub(crate) fn corrupt_installation_for_tests(&mut self, id: StylesheetSlotId) {
        self.slots.get_mut(&id).unwrap().installation_result =
            Some(Err(StylesheetInstallationFailure::HandoffAllocation));
    }
    pub(crate) fn finish_installation(
        &mut self,
        id: StylesheetSlotId,
        result: Result<(), StylesheetInstallationFailure>,
    ) {
        let slot = self.slots.get_mut(&id).expect("current attachment");
        assert!(slot.installation_result.is_none());
        assert!(matches!(slot.source, StylesheetSlotSource::External { .. }));
        assert!(slot.metadata_reserved && slot.body_reservation == BodyReservation::Issued);
        assert!(
            slot.fetch_result
                .as_ref()
                .is_some_and(StylesheetFetchResult::is_ready)
        );
        assert!(match result {
            Ok(()) => matches!(slot.state, StylesheetSlotState::Loaded(_)),
            Err(_) => matches!(
                slot.state,
                StylesheetSlotState::Pending | StylesheetSlotState::Loaded(_)
            ),
        });
        slot.installation_result = Some(result);
    }
    pub(crate) fn lifecycle_slots(
        &self,
    ) -> impl ExactSizeIterator<Item = StylesheetLifecycleView<'_>> + '_ {
        self.order.iter().map(|id| StylesheetLifecycleView {
            id: *id,
            slot: &self.slots[id],
        })
    }

    pub(crate) fn clear(&mut self) {
        self.slots.clear();
        self.order.clear();
        self.loaded_stylesheets.clear();
        self.source_bytes = 0;
        self.metadata_bytes = 0;
        self.body_bytes = 0;
    }

    #[cfg(test)]
    pub(crate) fn reconcile_from_dom(
        &mut self,
        dom: &Node,
        base: Option<&BrowserUrl>,
    ) -> StylesheetReconcileResult {
        let prepared = self
            .prepare_reconcile(dom, base, false)
            .expect("valid fixture inventory");
        self.commit_reconcile(prepared)
    }

    pub(crate) fn has_external_slot(&self, id: StylesheetSlotId) -> bool {
        #[cfg(test)]
        self.slot_lookups.set(self.slot_lookups.get() + 1);
        self.slots.get(&id).is_some_and(|slot| {
            matches!(
                &slot.source,
                StylesheetSlotSource::External {
                    address: Some(_),
                    ..
                }
            ) && matches!(
                slot.state,
                StylesheetSlotState::Pending | StylesheetSlotState::Loaded(_)
            )
        })
    }

    // Existing attachment-lifetime observation. It cannot authorize events or
    // enumerate issued requests; dispatch provenance lives in Tab.
    #[cfg(test)]
    pub(crate) fn active_external_slots(&self) -> HashSet<StylesheetSlotId> {
        self.ordered_slots()
            .filter_map(|(id, slot)| match &slot.source {
                StylesheetSlotSource::External {
                    address: Some(address),
                    ..
                } if address.capability(ResourceOperation::Retrieval)
                    == ResourceCapability::Supported =>
                {
                    Some(id)
                }
                _ => None,
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn slot_lookup_count(&self) -> usize {
        self.slot_lookups.get()
    }

    fn ordered_slots(&self) -> impl Iterator<Item = (StylesheetSlotId, &StylesheetSlot)> {
        self.order.iter().map(|id| (*id, &self.slots[id]))
    }

    #[cfg(test)]
    pub(crate) fn register_external_for_tests(&mut self, url: &str) -> StylesheetSlotId {
        let id = self.allocate_slot_id();
        self.order.push(id);
        self.slots.insert(
            id,
            StylesheetSlot {
                owner: Id(u32::try_from(id.0).expect("synthetic stylesheet owner")),
                source: StylesheetSlotSource::External {
                    href: url.to_string(),
                    href_present: true,
                    diagnostics: Vec::new(),
                    url_failure: None,
                    resource_base: None,
                    address: Some(ResourceAddress::new(
                        BrowserUrl::parse(url).expect("test URL").url,
                        ResourceKind::Stylesheet,
                    )),
                },
                media: None,
                host: HostEligibility::default(),
                link_label: LinkLabel::Absent,
                environment: None,
                decoding: None,
                runtime_receipt: None,
                state: StylesheetSlotState::Pending,
                fetch_result: None,
                installation_result: None,
                metadata_reserved: false,
                body_reservation: BodyReservation::None,
            },
        );
        id
    }

    #[cfg(test)]
    pub(crate) fn install_external_stylesheet(
        &mut self,
        id: StylesheetSlotId,
        text: &str,
        response: &net::NetworkResponseInfo,
    ) -> bool {
        self.install_external_parse(
            id,
            parse_stylesheet_with_options(text, &ParseOptions::stylesheet()),
            response,
        )
    }
    pub(crate) fn install_external_parse(
        &mut self,
        slot_id: StylesheetSlotId,
        parse: StylesheetParse,
        response: &net::NetworkResponseInfo,
    ) -> bool {
        assert_ne!(
            parse.completion(),
            css::StylesheetParseCompletion::InvariantViolation
        );
        let Some(slot) = self.slot_mut(slot_id) else {
            return false;
        };
        if !matches!(slot.source, StylesheetSlotSource::External { .. })
            || !matches!(
                slot.state,
                StylesheetSlotState::Pending | StylesheetSlotState::Loaded(_)
            )
        {
            return false;
        }
        if let StylesheetSlotSource::External { resource_base, .. } = &mut slot.source {
            *resource_base = response.response_url().cloned().map(Box::new);
        }
        slot.state = StylesheetSlotState::Loaded(parse);
        self.rebuild_loaded_stylesheets();
        true
    }

    pub(crate) fn mark_external_done(&mut self, slot_id: StylesheetSlotId) -> bool {
        if let Some(slot) = self.slot_mut(slot_id)
            && matches!(slot.state, StylesheetSlotState::Pending)
        {
            slot.state = StylesheetSlotState::Failed;
            return false;
        }
        false
    }

    pub(crate) fn mark_external_failed(&mut self, slot_id: StylesheetSlotId) -> bool {
        if let Some(slot) = self.slot_mut(slot_id) {
            let had_loaded_style = matches!(slot.state, StylesheetSlotState::Loaded(_));
            slot.state = StylesheetSlotState::Failed;
            if had_loaded_style {
                self.rebuild_loaded_stylesheets();
                return true;
            }
        }
        false
    }

    pub(crate) fn mark_external_aborted(&mut self, slot_id: StylesheetSlotId) -> bool {
        if let Some(slot) = self.slot_mut(slot_id) {
            let had_loaded_style = matches!(slot.state, StylesheetSlotState::Loaded(_));
            slot.state = StylesheetSlotState::Aborted;
            if had_loaded_style {
                self.rebuild_loaded_stylesheets();
                return true;
            }
        }
        false
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.slots
            .values()
            .filter(|slot| matches!(slot.state, StylesheetSlotState::Pending))
            .count()
    }

    pub(crate) fn stylesheets(&self) -> &[StylesheetParse] {
        &self.loaded_stylesheets
    }

    pub(crate) fn stylesheet_collection_inputs(
        &self,
    ) -> Result<Vec<StylesheetCollectionInput<'_>>, StylesheetCollectionInputBuildError> {
        let mut inputs = Vec::new();
        let input_capacity = self.slots.len().checked_add(1).ok_or(
            css::SourceCoordinateError::CounterExhausted {
                coordinate: "browser-stylesheet-input-count",
            },
        )?;
        inputs
            .try_reserve(input_capacity)
            .map_err(|_| StylesheetCollectionInputBuildError::Reservation)?;
        inputs.push(StylesheetCollectionInput::user_agent_for_namespace(
            StylesheetSourceId::built_in_user_agent(),
            StylesheetOrder::new(0),
            minimal_ua_stylesheet_parse(),
            html::ElementNamespace::Html,
        ));

        for (slot_index, (slot_id, slot)) in self.ordered_slots().enumerate() {
            let StylesheetSlotState::Loaded(stylesheet) = &slot.state else {
                continue;
            };
            if !slot.host.participates() || !complete(stylesheet) {
                continue;
            }
            let order_index =
                slot_index
                    .checked_add(1)
                    .ok_or(css::SourceCoordinateError::CounterExhausted {
                        coordinate: "browser-stylesheet-order",
                    })?;
            let order = StylesheetOrder::from_usize(order_index)?;
            let source_id = StylesheetSourceId::from_browser_slot(slot_id.0)?;
            inputs.push(StylesheetCollectionInput::author(
                source_id,
                order,
                stylesheet,
                StylesheetConditionInput::from_optional_raw_media(slot.media.as_deref()),
            ));
        }
        Ok(inputs)
    }

    /// Borrow actual available attachments, never the provenance-free flattened
    /// style projection. Pending/failed sources have no CSS input to observe.
    pub(crate) fn resource_sources(&self) -> impl Iterator<Item = StylesheetResourceSource<'_>> {
        self.ordered_slots().filter_map(|(slot_id, slot)| {
            let StylesheetSlotState::Loaded(parse) = &slot.state else {
                return None;
            };
            Some(StylesheetResourceSource {
                slot: slot_id,
                owner: slot.owner,
                parse,
                media: slot.media.as_deref(),
                base: match &slot.source {
                    StylesheetSlotSource::Inline(_) => StylesheetResourceBase::Document,
                    StylesheetSlotSource::External { resource_base, .. } => {
                        StylesheetResourceBase::Response(resource_base.as_deref())
                    }
                },
            })
        })
    }

    #[cfg(test)]
    fn allocate_slot_id(&mut self) -> StylesheetSlotId {
        self.next_slot_id = self
            .next_slot_id
            .checked_add(1)
            .expect("stylesheet slot id exhausted for document");
        StylesheetSlotId(self.next_slot_id)
    }

    fn slot_mut(&mut self, slot_id: StylesheetSlotId) -> Option<&mut StylesheetSlot> {
        #[cfg(test)]
        self.slot_lookups.set(self.slot_lookups.get() + 1);
        self.slots.get_mut(&slot_id)
    }

    fn rebuild_loaded_stylesheets(&mut self) {
        self.loaded_stylesheets.clear();
        let slots = &self.slots;
        self.loaded_stylesheets
            .extend(self.order.iter().filter_map(|id| {
                let slot = &slots[id];
                match &slot.state {
                    StylesheetSlotState::Loaded(parse)
                        if contributes(slot.host, slot.media.as_deref(), parse) =>
                    {
                        Some(clone_for_projection(parse))
                    }
                    _ => None,
                }
            }));
    }
}

fn minimal_ua_stylesheet_parse() -> &'static StylesheetParse {
    static MINIMAL_UA_STYLESHEET_PARSE: OnceLock<StylesheetParse> = OnceLock::new();

    MINIMAL_UA_STYLESHEET_PARSE.get_or_init(|| {
        parse_stylesheet_with_options(MINIMAL_UA_STYLESHEET, &ParseOptions::stylesheet())
    })
}

#[cfg(test)]
fn test_response() -> net::NetworkResponseInfo {
    let original = BrowserUrl::parse("https://example.com/a.css").unwrap().url;
    let mut headers = net::HttpHeaders::new();
    headers.append("content-type", b"text/css").unwrap();
    net::NetworkResponseInfo::http(
        &addressing::ResourceAddress::new(original, addressing::ResourceKind::Stylesheet),
        "https://example.com/a.css".into(),
        Some("https://example.com/a.css".into()),
        net::HttpStatus::try_from(200).unwrap(),
        headers,
    )
    .unwrap()
}

#[cfg(test)]
mod af5_tests {
    use super::*;
    use html::{HtmlParseOptions, parse_document};

    fn parsed(source: &str) -> html::ParseOutput {
        parse_document(source, HtmlParseOptions::default()).expect("HTML parses")
    }

    #[test]
    fn css_handoff_preserves_sparse_document_order_and_source_identity() {
        let output = parsed(concat!(
            "<!doctype html><html><head>",
            "<link rel=stylesheet href=a.css>",
            "<style>p { color: red; }</style>",
            "</head><body><p></p></body></html>"
        ));
        let mut set = DocumentStyleSet::default();
        let reconcile = set.reconcile_from_dom(
            &output.document,
            Some(
                &addressing::BrowserUrl::parse("https://example.com/")
                    .unwrap()
                    .url,
            ),
        );
        let external = reconcile.fetches[0];

        let first = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(first.len(), 2, "pending slot remains Browser-owned");
        assert_eq!(first[0].order().get(), 0);
        assert_eq!(
            first[1].order().get(),
            2,
            "available slots are not compacted"
        );
        let inline_source = first[1].source_id();

        let second = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(second[1].source_id(), inline_source);

        assert!(set.install_external_stylesheet(external, "p { color: blue; }", &test_response()));
        let complete = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(
            complete
                .iter()
                .map(|input| input.order().get())
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(complete[2].source_id(), inline_source);
        assert_ne!(complete[1].source_id(), complete[2].source_id());
    }

    #[test]
    fn exact_media_metadata_changes_reconciliation_and_is_not_parsed_by_browser() {
        let screen = parsed(
            "<!doctype html><html><head><style media='screen'>p { color: red; }</style></head><body></body></html>",
        );
        let print = parsed(
            "<!doctype html><html><head><style media='print'>p { color: red; }</style></head><body></body></html>",
        );
        let mut set = DocumentStyleSet::default();
        assert!(set.reconcile_from_dom(&screen.document, None).changed);
        let inputs = set.stylesheet_collection_inputs().unwrap();
        let source_id = inputs[1].source_id();
        assert_eq!(
            inputs[1].condition(),
            StylesheetConditionInput::RawMedia("screen")
        );
        assert!(set.reconcile_from_dom(&print.document, None).changed);
        let inputs = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(inputs[1].source_id(), source_id);
        assert_eq!(
            inputs[1].condition(),
            StylesheetConditionInput::RawMedia("print")
        );
    }

    #[test]
    fn loaded_external_media_change_preserves_slot_parse_and_source_without_refetch() {
        let screen = parsed(
            "<!doctype html><html><head><link rel=stylesheet href=a.css media=screen></head><body></body></html>",
        );
        let print = parsed(
            "<!doctype html><html><head><link rel=stylesheet href=a.css media=print></head><body></body></html>",
        );
        let mut set = DocumentStyleSet::default();
        let first = set.reconcile_from_dom(
            &screen.document,
            Some(
                &addressing::BrowserUrl::parse("https://example.com/")
                    .unwrap()
                    .url,
            ),
        );
        assert_eq!(first.fetches.len(), 1);
        let slot_id = first.fetches[0];
        assert!(set.install_external_stylesheet(slot_id, "p { color: red; }", &test_response()));
        let source_id = set.stylesheet_collection_inputs().unwrap()[1].source_id();

        let changed = set.reconcile_from_dom(
            &print.document,
            Some(
                &addressing::BrowserUrl::parse("https://example.com/")
                    .unwrap()
                    .url,
            ),
        );
        assert!(changed.changed);
        assert!(
            changed.fetches.is_empty(),
            "media-only change must not refetch"
        );
        assert!(
            set.stylesheets().is_empty(),
            "compatibility projection cannot carry deferred media"
        );
        assert!(
            set.lifecycle_slots().next().unwrap().has_loaded_parse(),
            "slot-owned parse remains inspectable"
        );
        let inputs = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(inputs[1].source_id(), source_id);
        assert_eq!(
            inputs[1].condition(),
            StylesheetConditionInput::RawMedia("print")
        );
    }

    #[test]
    fn duplicate_urls_keep_distinct_source_ids_and_completion_does_not_define_order() {
        let output = parsed(concat!(
            "<!doctype html><html><head>",
            "<link rel=stylesheet href=same.css>",
            "<link rel=stylesheet href=same.css>",
            "</head><body></body></html>",
        ));
        let mut set = DocumentStyleSet::default();
        let reconcile = set.reconcile_from_dom(
            &output.document,
            Some(
                &addressing::BrowserUrl::parse("https://example.com/")
                    .unwrap()
                    .url,
            ),
        );
        assert_eq!(reconcile.fetches.len(), 2);
        let first = reconcile.fetches[0];
        let second = reconcile.fetches[1];
        assert_ne!(first, second);

        assert!(set.install_external_stylesheet(second, "p { color: blue; }", &test_response()));
        let one_available = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(
            one_available
                .iter()
                .map(|input| input.order().get())
                .collect::<Vec<_>>(),
            vec![0, 2]
        );

        assert!(set.install_external_stylesheet(first, "p { color: red; }", &test_response()));
        let all_available = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(
            all_available
                .iter()
                .map(|input| input.order().get())
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_ne!(all_available[1].source_id(), all_available[2].source_id());
    }

    #[test]
    fn historical_style_scoped_attribute_is_ignored_and_contents_remain_global_author_css() {
        let output = parsed(concat!(
            "<!doctype html><html><body>",
            "<section><style scoped>p { color: red; }</style><p id=inside></p></section>",
            "<p id=outside></p>",
            "</body></html>",
        ));
        let mut set = DocumentStyleSet::default();
        assert!(set.reconcile_from_dom(&output.document, None).changed);
        let inputs = set
            .stylesheet_collection_inputs()
            .expect("ordinary style input remains representable");
        assert_eq!(inputs.len(), 2, "UA plus one ordinary author stylesheet");

        let diagnostic = css::rule_collection_diagnostic(
            &output.document,
            css::SelectorMatchingEnvironment::new(output.document_mode),
            &inputs,
            &css::StyleResolutionLimits::default(),
            css::RuleCollectionDiagnosticLimits::default(),
        );
        assert!(!diagnostic.to_debug_snapshot().contains("skipped-at-scope"));

        let resolved = css::try_resolve_document_styles_from_cascade_inputs_with_limits(
            &output.document,
            css::SelectorMatchingEnvironment::new(output.document_mode),
            &inputs,
            &css::StyleResolutionLimits::default(),
        )
        .expect("ordinary global author stylesheet resolves");
        let paragraph_colors = resolved
            .entries()
            .iter()
            .filter(|entry| entry.element_name() == "p")
            .map(|entry| {
                entry
                    .style()
                    .get(css::CascadePropertyId::Color)
                    .expect("both paragraphs receive a color")
                    .source()
            })
            .collect::<Vec<_>>();
        assert_eq!(paragraph_colors.len(), 2);
        assert!(paragraph_colors.iter().all(|source| matches!(
            source,
            css::ResolvedValueSource::Winner(winner)
                if winner.value.to_css_text().as_deref() == Some("red")
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::DocumentStyleSet;
    use html::internal::Id;

    fn element(
        id: u32,
        namespace: html::ElementNamespace,
        local_name: &str,
        attributes: Vec<(&str, &str)>,
        children: Vec<html::Node>,
    ) -> html::Node {
        html::internal::node_element_from_parts(
            Id(id),
            html::internal::expanded_name(namespace, local_name),
            attributes
                .into_iter()
                .map(|(name, value)| html::internal::unqualified_attribute(name, value))
                .collect(),
            Vec::new(),
            children,
        )
    }

    #[test]
    fn foreign_style_and_link_lookalikes_are_not_document_stylesheet_inputs() {
        let dom = html::Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![
                element(
                    2,
                    html::ElementNamespace::Svg,
                    "style",
                    Vec::new(),
                    vec![html::Node::Text {
                        id: Id(3),
                        text: "p { color: red; }".to_string(),
                    }],
                ),
                element(
                    4,
                    html::ElementNamespace::MathMl,
                    "link",
                    vec![("rel", "stylesheet"), ("href", "foreign.css")],
                    Vec::new(),
                ),
            ],
        };
        let mut set = DocumentStyleSet::default();
        let result = set.reconcile_from_dom(
            &dom,
            Some(
                &addressing::BrowserUrl::parse("https://example.test/")
                    .unwrap()
                    .url,
            ),
        );
        assert!(result.fetches.is_empty());
        assert!(set.stylesheets().is_empty());
    }
}

#[cfg(test)]
mod ah6_tests {
    use super::*;

    #[test]
    fn ah6_resolved_unsupported_sources_are_retained_without_request_descriptors() {
        let base = BrowserUrl::parse("https://example.test/page").unwrap().url;
        for reference in [
            "about:blank",
            "data:text/css,p{}",
            "blob:https://h.test/id",
            "ws://h.test/",
            "ftp://h.test/sheet",
            "custom:sheet",
        ] {
            let dom = html::parse_document(
                format!("<!doctype html><link rel=stylesheet href='{reference}'>"),
                Default::default(),
            )
            .unwrap()
            .document;
            let mut set = DocumentStyleSet::default();
            let result = set.reconcile_from_dom(&dom, Some(&base));
            assert!(result.fetches.is_empty());
            assert_eq!(set.pending_count(), 0);
            let (id, slot) = set.ordered_slots().next().unwrap();
            let StylesheetSlotSource::External {
                href,
                address: Some(address),
                ..
            } = &slot.source
            else {
                panic!("retained reference")
            };
            assert_eq!(href, reference);
            assert_eq!(address.url().as_str(), reference);
            assert_eq!(address.kind(), ResourceKind::Stylesheet);
            assert!(matches!(slot.state, StylesheetSlotState::Failed));
            assert!(!set.has_external_slot(id));
            assert!(set.active_external_slots().is_empty());
            assert!(set.lifecycle_slots().all(|slot| slot.is_failed()));
            let changed_base = BrowserUrl::parse("https://other.test/new/").unwrap().url;
            let again = set.reconcile_from_dom(&dom, Some(&changed_base));
            assert!(again.fetches.is_empty());
            assert!(again.retired.is_empty());
        }
    }
}

impl std::fmt::Debug for StylesheetSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StylesheetSlot")
            .field(
                "external",
                &matches!(self.source, StylesheetSlotSource::External { .. }),
            )
            .field("fetch", &self.fetch_result)
            .field("installation", &self.installation_result)
            .field("metadata_reserved", &self.metadata_reserved)
            .field("body_reservation", &self.body_reservation)
            .finish()
    }
}
impl std::fmt::Debug for DocumentStyleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentStyleSet")
            .field("attachments", &self.order.len())
            .field("loaded", &self.loaded_stylesheets.len())
            .field("source_bytes", &self.source_bytes)
            .field("metadata_bytes", &self.metadata_bytes)
            .field("body_bytes", &self.body_bytes)
            .finish()
    }
}
#[cfg(test)]
mod installation_transition_tests {
    use super::*;
    use crate::stylesheet_fetch::{
        StylesheetBodyEvidence, StylesheetFetchFailure, StylesheetFetchOutcome,
        StylesheetRequestEvidence, StylesheetResponseEvidence, StylesheetResponsePolicy,
    };
    #[test]
    fn aj7_installation_transition_refuses_unissued_or_unloaded_owners() {
        let mut set = DocumentStyleSet::default();
        let id = set.register_external_for_tests("https://example.test/s.css");
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| set
                .finish_installation(id, Err(StylesheetInstallationFailure::HandoffAllocation))))
            .is_err()
        );
        assert_eq!(set.installation_result(id), None);
        let request = net::FetchRequest::try_new(
            core_types::RequestId::default(),
            set.selected_address(id).unwrap().clone(),
            net::RequestMethod::Get,
            net::HttpHeaders::new(),
            None,
            None,
        )
        .unwrap();
        let issued_key = request.address().retrieval_key();
        let response = net::NetworkResponseInfo::http(
            request.address(),
            issued_key.as_str().into(),
            Some(issued_key.as_str().into()),
            net::HttpStatus::try_from(200).unwrap(),
            net::HttpHeaders::new(),
        )
        .unwrap();
        let policy = StylesheetResponsePolicy::classify(&response, &request);
        let mut fetch = StylesheetFetchResult::unissued(
            StylesheetRequestEvidence::Constructed {
                request,
                issued_key,
                enqueued: true,
                origin_allowance: 0,
            },
            StylesheetFetchFailure::DocumentAborted,
        );
        fetch.outcome = StylesheetFetchOutcome::Ready;
        fetch.response = StylesheetResponseEvidence::Selected(response);
        fetch.policy = Some(policy);
        fetch.body = StylesheetBodyEvidence::Complete(Vec::new());
        fetch.producer_progress =
            Some(net::BodyProgress::try_new(core_types::ResourceKind::Css, 0).unwrap());
        set.reserve_metadata(id).unwrap();
        set.provision_body(id).unwrap();
        set.issue_body(id);
        set.finish_fetch(id, fetch);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || set.finish_installation(id, Ok(()))
            ))
            .is_err()
        );
        assert_eq!(set.installation_result(id), None);
        set.finish_installation(id, Err(StylesheetInstallationFailure::HandoffAllocation));
        assert_eq!(
            set.installation_result(id),
            Some(Err(StylesheetInstallationFailure::HandoffAllocation))
        );
    }
}
