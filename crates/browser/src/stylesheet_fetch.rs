//! Browser-owned stylesheet acquisition evidence, before character decoding.
//!
//! Payload charges are logical retention admission, never allocator/RSS bounds.
use addressing::{BrowserUrl, ResourceIdentityKey, UrlDiagnostic, UrlFailure, UrlParseOutcome};
use net::{BodyProgress, FetchRequest, NetworkErrorKind, NetworkResponseInfo};
use std::fmt;

pub(crate) const BODY_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const BODY_POOL: usize = 64 * 1024 * 1024;
pub(crate) const METADATA_BYTES: usize = 256 * 1024;
pub(crate) const METADATA_POOL: usize = 8 * 1024 * 1024;
pub(crate) const SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const ATTACHMENTS: usize = 4096;
pub(crate) const ORDINARY_NODES: usize = 65_537;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetInventoryStorage {
    Traversal,
    Owners,
    Entries,
    Source,
    Destination,
    Order,
    Projection,
    Retirement,
    Dispatch,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetInventoryError {
    AttachmentLimit,
    SourceLimit,
    OrdinaryNodeLimit,
    ArithmeticOverflow,
    SlotIdExhausted,
    Reservation(StylesheetInventoryStorage),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetRetentionFailure {
    BodyReservationDenied,
    MetadataReservationDenied,
    RequestMetadataLimit,
    ResponseMetadataLimit,
    RequestRecordAllocation,
    BodyStorageAllocation,
    BodyLimit,
    ArithmeticOverflow,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetSourceFailure {
    MissingHref,
    EmptyHref,
    EmptyAfterPreprocessing,
    BaseUnavailable,
    Url(addressing::UrlFailureKind),
    UnsupportedRetrieval(addressing::ResourceCapability),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetProtocolFailure {
    RequestIdentity,
    ChunkBeforeStart,
    IssuedKey,
    CompletionBeforeStart,
    CompletionMetadata,
    BodyRoute,
    BodyCount,
    NullBodyBytes,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetFetchFailure {
    Source(StylesheetSourceFailure),
    RequestAdmission(net::RequestAdmissionError),
    RequestDeliveryUnavailable,
    Network(NetworkErrorKind),
    Retention(StylesheetRetentionFailure),
    Protocol(StylesheetProtocolFailure),
    DocumentAborted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetResponseRejection {
    MissingFinalUrl,
    InvalidFinalUrl,
    IncompatibleFinalScheme,
    NoBodyStatus(u16),
    PartialRepresentationUnsupported,
    InstanceManipulationUnsupported,
    NotModifiedWithoutRepresentation,
    HttpStatus(u16),
    UnusableContentType,
    UnsupportedContentType,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetFetchOutcome {
    Ready,
    Rejected(StylesheetResponseRejection),
    Failed(StylesheetFetchFailure),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetEvidenceOmission {
    MetadataLimit,
    UnauthenticatedEvent,
    ConflictingContext,
    InconsistentProgress,
    CollectionStopped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetCollectionStop {
    Network,
    Protocol,
    Retention,
    DocumentAborted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetErrorContext {
    NotApplicable,
    Absent,
    Contextual,
    MatchesSelected,
    Discarded,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetInstallationFailure {
    HandoffAllocation,
    ChunkDeliveryUnavailable,
    DoneDeliveryUnavailable,
    RuntimeProvenanceMismatch,
    UnexpectedRuntimeEvent,
}

// Keep request ownership inline; adding Box would add infallible per-record growth.
#[allow(clippy::large_enum_variant)]
pub enum StylesheetRequestEvidence {
    NotConstructed,
    ConstructedOmitted(StylesheetEvidenceOmission),
    Constructed {
        request: FetchRequest,
        issued_key: ResourceIdentityKey,
        enqueued: bool,
        origin_allowance: usize,
    },
}
impl StylesheetRequestEvidence {
    pub fn request(&self) -> Option<&FetchRequest> {
        match self {
            Self::Constructed { request, .. } => Some(request),
            Self::NotConstructed | Self::ConstructedOmitted(_) => None,
        }
    }
    pub fn issued_key(&self) -> Option<&ResourceIdentityKey> {
        match self {
            Self::Constructed { issued_key, .. } => Some(issued_key),
            Self::NotConstructed | Self::ConstructedOmitted(_) => None,
        }
    }
    pub fn omission(&self) -> Option<StylesheetEvidenceOmission> {
        match self {
            Self::ConstructedOmitted(reason) => Some(*reason),
            _ => None,
        }
    }
    pub fn enqueued(&self) -> bool {
        matches!(self, Self::Constructed { enqueued: true, .. })
    }
    pub(crate) fn origin_allowance(&self) -> usize {
        match self {
            Self::Constructed {
                origin_allowance, ..
            } => *origin_allowance,
            Self::NotConstructed | Self::ConstructedOmitted(_) => 0,
        }
    }
}
impl fmt::Debug for StylesheetRequestEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StylesheetRequestEvidence")
            .field("constructed", &!matches!(self, Self::NotConstructed))
            .field("omission", &self.omission())
            .field("enqueued", &self.enqueued())
            .finish()
    }
}
pub enum StylesheetResponseEvidence {
    Absent,
    Selected(NetworkResponseInfo),
    Contextual(NetworkResponseInfo),
    Unavailable(StylesheetEvidenceOmission),
}
impl StylesheetResponseEvidence {
    pub fn response(&self) -> Option<&NetworkResponseInfo> {
        match self {
            Self::Selected(r) | Self::Contextual(r) => Some(r),
            _ => None,
        }
    }
}
impl fmt::Debug for StylesheetResponseEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Absent => "Absent",
            Self::Selected(_) => "Selected",
            Self::Contextual(_) => "Contextual",
            Self::Unavailable(reason) => {
                return f.debug_tuple("Unavailable").field(reason).finish();
            }
        })
    }
}
pub enum StylesheetBodyEvidence {
    Absent,
    NotStarted,
    Null,
    Complete(Vec<u8>),
    Partial {
        bytes: Vec<u8>,
        observed_bytes: usize,
        unretained_event_bytes: usize,
        stop: StylesheetCollectionStop,
    },
    Unavailable(StylesheetEvidenceOmission),
}
impl StylesheetBodyEvidence {
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Complete(v) | Self::Partial { bytes: v, .. } => Some(v),
            _ => None,
        }
    }
    pub fn category(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::NotStarted => "not-started",
            Self::Null => "null",
            Self::Complete(_) => "complete",
            Self::Partial { .. } => "partial",
            Self::Unavailable(_) => "unavailable",
        }
    }
}
impl fmt::Debug for StylesheetBodyEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        {
            let mut d = f.debug_struct("StylesheetBodyEvidence");
            d.field("kind", &self.category())
                .field("retained_bytes", &self.bytes().map(<[u8]>::len));
            match self {
                Self::Partial {
                    observed_bytes,
                    unretained_event_bytes,
                    stop,
                    ..
                } => {
                    d.field("observed_bytes", observed_bytes)
                        .field("unretained_event_bytes", unretained_event_bytes)
                        .field("stop", stop);
                }
                Self::Unavailable(reason) => {
                    d.field("reason", reason);
                }
                _ => {}
            }
            d.finish()
        }
    }
}

/// Scalar applicability facts survive a later transport failure independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StylesheetResponsePolicy {
    pub provenance: Option<StylesheetResponseRejection>,
    pub status: Option<StylesheetResponseRejection>,
    pub mime: Option<StylesheetResponseRejection>,
}
impl StylesheetResponsePolicy {
    pub(crate) fn classify(response: &NetworkResponseInfo, request: &FetchRequest) -> Self {
        use StylesheetResponseRejection::*;
        let provenance = match response.response_url() {
            None => Some(MissingFinalUrl),
            Some(Err(_)) => Some(InvalidFinalUrl),
            Some(Ok(p)) => {
                let valid = match request.original_url().scheme() {
                    "http" | "https" => {
                        response.status().is_some() && matches!(p.url.scheme(), "http" | "https")
                    }
                    "file" => response.status().is_none() && p.url.scheme() == "file",
                    _ => false,
                };
                (!valid).then_some(IncompatibleFinalScheme)
            }
        };
        let status = match response.status_code() {
            Some(code @ (204 | 205)) => Some(NoBodyStatus(code)),
            Some(206) => Some(PartialRepresentationUnsupported),
            Some(226) => Some(InstanceManipulationUnsupported),
            Some(304) => Some(NotModifiedWithoutRepresentation),
            Some(200..=299) | None => None,
            Some(code) => Some(HttpStatus(code)),
        };
        let mime = match response.content_type() {
            net::ResponseContentType::Missing | net::ResponseContentType::Empty => None,
            net::ResponseContentType::Unusable => Some(UnusableContentType),
            net::ResponseContentType::Parsed(p) if p.mime_type() == "text/css" => None,
            _ => Some(UnsupportedContentType),
        };
        Self {
            provenance,
            status,
            mime,
        }
    }
    pub fn rejection(self) -> Option<StylesheetResponseRejection> {
        self.provenance.or(self.status).or(self.mime)
    }
}

pub struct StylesheetFetchResult {
    pub(crate) request: StylesheetRequestEvidence,
    pub(crate) response: StylesheetResponseEvidence,
    pub(crate) policy: Option<StylesheetResponsePolicy>,
    pub(crate) body: StylesheetBodyEvidence,
    pub(crate) producer_progress: Option<BodyProgress>,
    pub(crate) acquisition_error: Option<NetworkErrorKind>,
    // Bytes rejected by an event-authentication/phase gate are not admitted body bytes.
    pub(crate) discarded_event_bytes: usize,
    pub(crate) discarded_event_reason: Option<StylesheetProtocolFailure>,
    pub(crate) outcome: StylesheetFetchOutcome,
    pub(crate) error_context: StylesheetErrorContext,
    pub(crate) context_omission: Option<StylesheetEvidenceOmission>,
    pub(crate) progress_omission: Option<StylesheetEvidenceOmission>,
}
impl StylesheetFetchResult {
    pub(crate) fn unissued(
        request: StylesheetRequestEvidence,
        failure: StylesheetFetchFailure,
    ) -> Self {
        Self {
            request,
            response: StylesheetResponseEvidence::Absent,
            policy: None,
            body: StylesheetBodyEvidence::Absent,
            producer_progress: None,
            acquisition_error: None,
            discarded_event_bytes: 0,
            discarded_event_reason: None,
            outcome: StylesheetFetchOutcome::Failed(failure),
            error_context: StylesheetErrorContext::NotApplicable,
            context_omission: None,
            progress_omission: None,
        }
    }
    pub fn request(&self) -> &StylesheetRequestEvidence {
        &self.request
    }
    pub fn response(&self) -> &StylesheetResponseEvidence {
        &self.response
    }
    pub fn policy(&self) -> Option<StylesheetResponsePolicy> {
        self.policy
    }
    pub fn body(&self) -> &StylesheetBodyEvidence {
        &self.body
    }
    pub fn outcome(&self) -> StylesheetFetchOutcome {
        self.outcome
    }
    pub fn producer_progress(&self) -> Option<BodyProgress> {
        self.producer_progress
    }
    pub fn discarded_event_bytes(&self) -> usize {
        self.discarded_event_bytes
    }
    pub fn discarded_event_reason(&self) -> Option<StylesheetProtocolFailure> {
        self.discarded_event_reason
    }
    pub fn acquisition_error(&self) -> Option<NetworkErrorKind> {
        self.acquisition_error
    }
    pub fn error_context(&self) -> StylesheetErrorContext {
        self.error_context
    }
    pub fn context_omission(&self) -> Option<StylesheetEvidenceOmission> {
        self.context_omission
    }
    pub fn progress_omission(&self) -> Option<StylesheetEvidenceOmission> {
        self.progress_omission
    }
}
impl fmt::Debug for StylesheetFetchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StylesheetFetchResult")
            .field("request", &self.request)
            .field("response", &self.response)
            .field("policy", &self.policy)
            .field("body", &self.body)
            .field("producer_progress", &self.producer_progress)
            .field("acquisition_error", &self.acquisition_error())
            .field("discarded_event_bytes", &self.discarded_event_bytes())
            .field("discarded_event_reason", &self.discarded_event_reason())
            .field("outcome", &self.outcome)
            .field("error_context", &self.error_context())
            .field("context_omission", &self.context_omission())
            .field("progress_omission", &self.progress_omission())
            .finish()
    }
}

pub(crate) struct StylesheetFetchAccumulator {
    pub(crate) request: FetchRequest,
    pub(crate) issued_key: ResourceIdentityKey,
    pub(crate) origin_allowance: usize,
    pub(crate) response: Option<NetworkResponseInfo>,
    pub(crate) policy: Option<StylesheetResponsePolicy>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) observed_bytes: usize,
}
impl StylesheetFetchAccumulator {
    #[allow(clippy::result_large_err)] // Move failure evidence inline; no extra allocation on refusal.
    pub(crate) fn try_new(
        request: FetchRequest,
        origin_allowance: usize,
    ) -> Result<Self, StylesheetFetchResult> {
        if metadata_charge(&request, origin_allowance, None)
            .is_none_or(|charge| charge > METADATA_BYTES)
        {
            // The constructed request is deliberately dropped, never retained
            // beyond the independently admitted metadata credit.
            return Err(StylesheetFetchResult::unissued(
                StylesheetRequestEvidence::ConstructedOmitted(
                    StylesheetEvidenceOmission::MetadataLimit,
                ),
                StylesheetFetchFailure::Retention(StylesheetRetentionFailure::RequestMetadataLimit),
            ));
        }
        let issued_key = request.address().retrieval_key();
        Ok(Self {
            request,
            issued_key,
            origin_allowance,
            response: None,
            policy: None,
            bytes: Vec::new(),
            observed_bytes: 0,
        })
    }
    pub(crate) fn identity_matches(&self, r: &NetworkResponseInfo) -> bool {
        r.resource_kind() == addressing::ResourceKind::Stylesheet
            && r.original_url() == self.request.original_url()
            && r.requested_url() == self.issued_key.as_str()
    }
    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<(), StylesheetRetentionFailure> {
        let total = self
            .observed_bytes
            .checked_add(bytes.len())
            .ok_or(StylesheetRetentionFailure::ArithmeticOverflow)?;
        self.observed_bytes = total;
        if total > BODY_BYTES {
            return Err(StylesheetRetentionFailure::BodyLimit);
        }
        #[cfg(test)]
        if allocation_refused(AllocationBoundary::Body) {
            return Err(StylesheetRetentionFailure::BodyStorageAllocation);
        }
        if self.bytes.capacity() < total {
            let target = self
                .bytes
                .capacity()
                .max(4096)
                .saturating_mul(2)
                .max(total)
                .min(BODY_BYTES);
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| StylesheetRetentionFailure::BodyStorageAllocation)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub(crate) fn finish(
        self,
        outcome: StylesheetFetchOutcome,
        progress: Option<BodyProgress>,
        stop: StylesheetCollectionStop,
    ) -> StylesheetFetchResult {
        let started = self.response.is_some();
        let body = if matches!(
            outcome,
            StylesheetFetchOutcome::Ready | StylesheetFetchOutcome::Rejected(_)
        ) {
            if self
                .response
                .as_ref()
                .and_then(NetworkResponseInfo::status)
                .is_some_and(|s| s.has_null_body())
            {
                StylesheetBodyEvidence::Null
            } else {
                StylesheetBodyEvidence::Complete(self.bytes)
            }
        } else if started {
            let unretained_event_bytes = self.observed_bytes.saturating_sub(self.bytes.len());
            StylesheetBodyEvidence::Partial {
                bytes: self.bytes,
                observed_bytes: self.observed_bytes,
                unretained_event_bytes,
                stop,
            }
        } else {
            StylesheetBodyEvidence::NotStarted
        };
        StylesheetFetchResult {
            request: StylesheetRequestEvidence::Constructed {
                request: self.request,
                issued_key: self.issued_key,
                enqueued: true,
                origin_allowance: self.origin_allowance,
            },
            response: self.response.map_or(
                StylesheetResponseEvidence::Absent,
                StylesheetResponseEvidence::Selected,
            ),
            policy: self.policy,
            body,
            producer_progress: progress,
            acquisition_error: None,
            discarded_event_bytes: 0,
            discarded_event_reason: None,
            outcome,
            error_context: StylesheetErrorContext::NotApplicable,
            context_omission: None,
            progress_omission: None,
        }
    }
}

fn sum(values: impl IntoIterator<Item = usize>) -> Option<usize> {
    values.into_iter().try_fold(0usize, usize::checked_add)
}
pub(crate) fn outcome_charge(outcome: &Result<UrlParseOutcome, UrlFailure>) -> Option<usize> {
    match outcome {
        Ok(p) => sum([
            p.url.as_str().len(),
            p.diagnostics
                .len()
                .checked_mul(std::mem::size_of::<UrlDiagnostic>())?,
        ]),
        Err(e) => sum([
            e.base.as_ref().map_or(0, |b| b.as_str().len()),
            e.diagnostics
                .len()
                .checked_mul(std::mem::size_of::<UrlDiagnostic>())?,
        ]),
    }
}
pub(crate) fn header_charge(headers: &net::HttpHeaders) -> Option<usize> {
    headers
        .iter()
        .try_fold(0usize, |n, (k, v)| sum([n, k.len(), v.len()]))
}
pub(crate) fn request_charge(request: &FetchRequest, origin_allowance: usize) -> Option<usize> {
    sum([
        request.original_url().as_str().len(),
        request.current_url().as_str().len(),
        header_charge(request.headers())?,
        request.initiator_url().map_or(0, |u| u.as_str().len()),
        origin_allowance,
    ])
}
pub(crate) fn response_charge(r: &NetworkResponseInfo) -> Option<usize> {
    let mime = match r.content_type() {
        net::ResponseContentType::Parsed(c) => {
            sum([c.mime_type().len(), c.charset().map_or(0, str::len)])?
        }
        _ => 0,
    };
    let redirects = r.redirect_chain().iter().try_fold(0usize, |n, h| {
        sum([
            n,
            h.request_url().as_str().len(),
            h.response_url().as_str().len(),
            h.target_url().as_str().len(),
        ])
    })?;
    sum([
        r.original_url().as_str().len(),
        r.requested_url().len(),
        r.reported_response_url().map_or(0, str::len),
        r.response_url().map_or(Some(0), outcome_charge)?,
        header_charge(r.headers())?,
        mime,
        if r.status().is_none() { 8192 } else { 0 },
        redirects,
    ])
}
pub(crate) fn metadata_charge(
    request: &FetchRequest,
    origin_allowance: usize,
    response: Option<&NetworkResponseInfo>,
) -> Option<usize> {
    // Two complete-original allowances conservatively cover key + abort URL.
    let base = sum([
        request_charge(request, origin_allowance)?,
        request.original_url().as_str().len().checked_mul(2)?,
    ])?;
    match response {
        None => Some(base),
        Some(r) => sum([
            base,
            response_charge(r)?,
            r.response_url().map_or(Some(0), outcome_charge)?,
        ]),
    }
}
pub(crate) fn request_charge_before_copy(
    address: &BrowserUrl,
    initiator: Option<&BrowserUrl>,
    opaque_origin: bool,
) -> Option<usize> {
    let i = initiator.map_or(0, |u| u.as_str().len());
    let origin = if opaque_origin { 0 } else { i.checked_add(32)? };
    sum([
        address.as_str().len().checked_mul(4)?,
        i,
        origin,
        if matches!(address.scheme(), "http" | "https") {
            9
        } else {
            0
        },
    ])
}
pub(crate) fn inventory_growth(
    storage: StylesheetInventoryStorage,
) -> Result<(), StylesheetInventoryError> {
    #[cfg(test)]
    if allocation_refused(AllocationBoundary::Inventory(storage)) {
        if REFUSE_INVENTORY_WITH_ALLOCATOR.with(|mode| mode.get()) {
            crate::tab::tests::parsed_document_observation::arm_inventory_allocation_refusal();
        } else {
            return Err(StylesheetInventoryError::Reservation(storage));
        }
    }
    let _ = storage;
    Ok(())
}
pub(crate) fn try_string(value: &str) -> Result<String, StylesheetInventoryError> {
    if !value.is_empty() {
        inventory_growth(StylesheetInventoryStorage::Source)?;
    }
    let mut out = String::new();
    out.try_reserve_exact(value.len())
        .map_err(|_| StylesheetInventoryError::Reservation(StylesheetInventoryStorage::Source))?;
    out.push_str(value);
    Ok(out)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AllocationBoundary {
    Inventory(StylesheetInventoryStorage),
    RequestRecord,
    Body,
    Handoff,
}
#[cfg(test)]
thread_local! {pub(crate) static REFUSE_ALLOCATION: std::cell::Cell<Option<AllocationBoundary>>=const{std::cell::Cell::new(None)};}
#[cfg(test)]
thread_local! {pub(crate) static REFUSE_INVENTORY_WITH_ALLOCATOR:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
#[cfg(test)]
thread_local! {pub(crate) static REFUSE_MATCHING_ALLOCATION_AFTER:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
pub(crate) fn allocation_refused(point: AllocationBoundary) -> bool {
    REFUSE_ALLOCATION.with(|v| {
        if v.get() == Some(point) {
            if REFUSE_MATCHING_ALLOCATION_AFTER.with(|remaining| {
                let n = remaining.get();
                if n > 0 {
                    remaining.set(n - 1);
                    true
                } else {
                    false
                }
            }) {
                return false;
            }
            v.set(None);
            true
        } else {
            false
        }
    })
}
impl StylesheetFetchResult {
    /// Pure validation of the facts admitted by the acquisition owner. The AH
    /// issued key is recorded once; this does not reparse or reconstruct URLs.
    pub(crate) fn coherent_evidence(&self) -> bool {
        let Some(request) = self.request.request() else {
            if let StylesheetRequestEvidence::ConstructedOmitted(reason) = self.request
                && (reason != StylesheetEvidenceOmission::MetadataLimit
                    || self.outcome
                        != StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
                            StylesheetRetentionFailure::RequestMetadataLimit,
                        )))
            {
                return false;
            }
            return matches!(
                self.outcome,
                StylesheetFetchOutcome::Failed(
                    StylesheetFetchFailure::Source(_)
                        | StylesheetFetchFailure::RequestAdmission(_)
                        | StylesheetFetchFailure::Retention(_)
                )
            ) && matches!(self.response, StylesheetResponseEvidence::Absent)
                && self.policy.is_none()
                && matches!(self.body, StylesheetBodyEvidence::Absent)
                && self.producer_progress.is_none()
                && self.acquisition_error.is_none()
                && self.error_context == StylesheetErrorContext::NotApplicable
                && self.context_omission.is_none()
                && self.progress_omission.is_none()
                && self.discarded_event_bytes == 0
                && self.discarded_event_reason.is_none();
        };
        if request.address().kind() != addressing::ResourceKind::Stylesheet
            || request.current_url() != request.original_url()
        {
            return false;
        }
        let Some(key) = self.request.issued_key() else {
            return false;
        };
        if !self.request.enqueued() {
            return matches!(
                self.outcome,
                StylesheetFetchOutcome::Failed(
                    StylesheetFetchFailure::RequestAdmission(_)
                        | StylesheetFetchFailure::RequestDeliveryUnavailable
                        | StylesheetFetchFailure::Retention(_)
                )
            ) && matches!(self.response, StylesheetResponseEvidence::Absent)
                && self.policy.is_none()
                && matches!(self.body, StylesheetBodyEvidence::Absent)
                && self.producer_progress.is_none()
                && self.acquisition_error.is_none()
                && self.context_omission.is_none()
                && self.progress_omission.is_none()
                && self.error_context == StylesheetErrorContext::NotApplicable
                && self.discarded_event_bytes == 0
                && self.discarded_event_reason.is_none();
        }
        if matches!(
            self.outcome,
            StylesheetFetchOutcome::Failed(
                StylesheetFetchFailure::Source(_)
                    | StylesheetFetchFailure::RequestAdmission(_)
                    | StylesheetFetchFailure::RequestDeliveryUnavailable
            )
        ) {
            return false;
        }
        let identity = |response: &NetworkResponseInfo| {
            response.resource_kind() == addressing::ResourceKind::Stylesheet
                && response.original_url() == request.original_url()
                && response.requested_url() == key.as_str()
        };
        let selected = match &self.response {
            StylesheetResponseEvidence::Selected(response) => {
                if !identity(response)
                    || self.policy != Some(StylesheetResponsePolicy::classify(response, request))
                {
                    return false;
                }
                Some(response)
            }
            StylesheetResponseEvidence::Contextual(response) => {
                if !identity(response)
                    || self.policy.is_some()
                    || self.error_context != StylesheetErrorContext::Contextual
                    || self.acquisition_error.is_none()
                    || self.context_omission.is_some()
                {
                    return false;
                }
                None
            }
            StylesheetResponseEvidence::Absent => {
                if self.policy.is_some() {
                    return false;
                }
                None
            }
            StylesheetResponseEvidence::Unavailable(reason) => {
                if self.policy.is_some() || self.context_omission != Some(*reason) {
                    return false;
                }
                None
            }
        };
        let completed = matches!(
            self.outcome,
            StylesheetFetchOutcome::Ready | StylesheetFetchOutcome::Rejected(_)
        );
        let observed = match &self.body {
            StylesheetBodyEvidence::Complete(bytes) => {
                if !completed
                    || selected.is_none()
                    || bytes.len() > BODY_BYTES
                    || selected
                        .and_then(NetworkResponseInfo::status)
                        .is_some_and(|status| status.has_null_body())
                {
                    return false;
                }
                bytes.len()
            }
            StylesheetBodyEvidence::Null => {
                if !completed
                    || !selected
                        .and_then(NetworkResponseInfo::status)
                        .is_some_and(|status| status.has_null_body())
                {
                    return false;
                }
                0
            }
            StylesheetBodyEvidence::Partial {
                bytes,
                observed_bytes,
                unretained_event_bytes,
                stop,
            } => {
                let expected_stop = match self.outcome {
                    StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(_)) => {
                        Some(StylesheetCollectionStop::Network)
                    }
                    StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(_)) => {
                        Some(StylesheetCollectionStop::Protocol)
                    }
                    StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(_)) => {
                        Some(StylesheetCollectionStop::Retention)
                    }
                    StylesheetFetchOutcome::Failed(StylesheetFetchFailure::DocumentAborted) => {
                        Some(StylesheetCollectionStop::DocumentAborted)
                    }
                    _ => None,
                };
                if expected_stop != Some(*stop) {
                    return false;
                }
                if completed
                    || selected.is_none()
                    || selected
                        .and_then(NetworkResponseInfo::status)
                        .is_some_and(|status| status.has_null_body())
                        && *observed_bytes != 0
                    || bytes.len() > BODY_BYTES
                    || bytes.len() > *observed_bytes
                    || observed_bytes.checked_sub(bytes.len()) != Some(*unretained_event_bytes)
                {
                    return false;
                }
                *observed_bytes
            }
            StylesheetBodyEvidence::NotStarted => {
                if completed || selected.is_some() {
                    return false;
                }
                0
            }
            StylesheetBodyEvidence::Unavailable(_) => {
                if completed || !matches!(self.response, StylesheetResponseEvidence::Unavailable(_))
                {
                    return false;
                }
                0
            }
            StylesheetBodyEvidence::Absent => return false,
        };
        if self.producer_progress.is_some_and(|progress| {
            !progress.matches_route(core_types::ResourceKind::Css)
                || progress.delivered_bytes() != observed
        }) {
            return false;
        }
        if self.progress_omission.is_some() && self.producer_progress.is_some() {
            return false;
        }
        if self.discarded_event_bytes > 0 && self.discarded_event_reason.is_none() {
            return false;
        }
        if self.discarded_event_reason.is_some()
            && !matches!(
                self.outcome,
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(_))
            )
        {
            return false;
        }
        match self.outcome {
            StylesheetFetchOutcome::Ready | StylesheetFetchOutcome::Rejected(_) => {
                let rejection = self.policy.and_then(StylesheetResponsePolicy::rejection);
                let policy_matches = match self.outcome {
                    StylesheetFetchOutcome::Ready => rejection.is_none(),
                    StylesheetFetchOutcome::Rejected(reason) => rejection == Some(reason),
                    _ => false,
                };
                policy_matches
                    && selected.is_some()
                    && self.producer_progress.is_some()
                    && self.acquisition_error.is_none()
                    && self.error_context == StylesheetErrorContext::NotApplicable
                    && self.context_omission.is_none()
                    && self.progress_omission.is_none()
                    && self.discarded_event_bytes == 0
                    && self.discarded_event_reason.is_none()
                    && (!matches!(self.outcome, StylesheetFetchOutcome::Ready)
                        || matches!(self.body, StylesheetBodyEvidence::Complete(_)))
            }
            StylesheetFetchOutcome::Failed(failure) => match failure {
                StylesheetFetchFailure::Network(kind) => self.acquisition_error == Some(kind),
                _ => true,
            },
        }
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.outcome == StylesheetFetchOutcome::Ready && self.coherent_evidence()
    }
}
