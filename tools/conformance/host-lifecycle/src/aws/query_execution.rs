//! SDK-independent logical-query mechanics, private to the closed AWS boundary.
//! No clients, request closures, dispatch, discovery or policy live here.
use super::response_limits::ObservationRound;
use super::{
    ec2_decode_integrity::allocation::QualifiedAllocationReceipt, ec2_observation::combine_failure,
};
use crate::provider::{
    ec2_allocation_observation_v5::ObservationDataV5,
    ec2_observation_v4::FactsV4,
    observation_v5::ObservationRecordV5,
    source_occurrence_v5::{SourceCreditV5, SourceOccurrenceV5, SourcePageV5, SourcePathV5},
};
use crate::{
    canonical,
    provider::{coverage::*, limits::*, observation_v5::ObservationEntryV5},
};
use std::collections::BTreeSet;
use std::sync::Arc;

/// An unforgeable query-local attempt. Only a completed integrity receiver can return its receipt.
pub(super) struct AllocationAttempt {
    stamp: Arc<()>,
    operation: ReadOperationV1,
}
impl AllocationAttempt {
    pub(super) fn operation(&self) -> ReadOperationV1 {
        self.operation
    }
    pub(super) fn matches(&self, stamp: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.stamp, stamp)
    }
}

type ReadResult<T> = std::result::Result<T, ReadFailureV1>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pagination {
    Singleton,
    Token,
}

/// The operation/scope allowlist is distinct from frozen QueryIdentityV1 validation.
/// Adding an adapter cannot accidentally grant a new query scope.
pub(super) fn pagination(query: &QueryIdentityV1) -> ReadResult<Pagination> {
    use crate::provider::inventory::ResourceIdentity as R;
    use QueryScopeV1 as S;
    use ReadOperationV1 as O;
    query.validate().map_err(|_| ReadFailureV1::Malformed)?;
    let exact = |predicate: fn(&R) -> bool| matches!(&query.scope, S::Exact { identities } if identities.iter().all(predicate));
    let single = |predicate: fn(&R) -> bool| matches!(&query.scope, S::Exact { identities } if identities.len() == 1 && predicate(&identities[0]));
    let regional = matches!(query.scope, S::Regional);
    let discovery = matches!(
        query.scope,
        S::OperationTags { .. } | S::AuthorityTag { .. } | S::OperationTag { .. }
    );
    let valid = match query.operation {
        O::GetCallerIdentity => regional,
        O::HeadBucket => single(|r| matches!(r, R::Bucket(_))),
        O::GetInstanceProfile => single(|r| matches!(r, R::Profile(_))),
        O::DescribeKey => single(|r| matches!(r, R::Key(_))),
        O::DescribeRegions | O::DescribeAvailabilityZones => regional,
        O::DescribeSubnets => exact(|r| matches!(r, R::Subnet(_))),
        O::DescribeVpcs => exact(|r| matches!(r, R::Vpc(_))),
        O::DescribeSecurityGroups => exact(|r| matches!(r, R::SecurityGroup(_))),
        O::DescribeRouteTables => exact(|r| matches!(r, R::RouteTable(_))),
        O::DescribeVpcEndpoints => exact(|r| matches!(r, R::Endpoint(_))),
        O::DescribePrefixLists => exact(|r| matches!(r, R::PrefixList(_))),
        O::DescribeDhcpOptions => exact(|r| matches!(r, R::Dhcp(_))),
        O::DescribeNetworkAcls => exact(|r| matches!(r, R::Nacl(_))),
        O::DescribeImages => exact(|r| matches!(r, R::Image(_))),
        O::DescribeInstanceTypes => exact(|r| matches!(r, R::InstanceType(_))),
        O::DescribeInstanceTypeOfferings => matches!(query.scope, S::TypeOffering { .. }),
        O::DescribeVpcAttribute => matches!(query.scope, S::VpcAttribute { .. }),
        O::DescribeInstanceAttribute => matches!(query.scope, S::InstanceAttribute { .. }),
        O::DescribeIamInstanceProfileAssociations => {
            exact(|r| matches!(r, R::ProfileAssociation(_)))
                || matches!(query.scope, S::AttachedTo { .. })
        }
        O::DescribeInstances => {
            exact(|r| matches!(r, R::Instance(_)))
                || discovery
                || matches!(query.scope, S::ClientToken { .. })
        }
        O::DescribeNetworkInterfaces => {
            exact(|r| matches!(r, R::NetworkInterface(_)))
                || discovery
                || matches!(query.scope, S::AttachedTo { .. })
        }
        O::DescribeVolumes => {
            exact(|r| matches!(r, R::Volume(_)))
                || discovery
                || matches!(query.scope, S::AttachedTo { .. })
        }
    };
    if !valid {
        return Err(ReadFailureV1::Unsupported);
    }
    Ok(match query.operation {
        O::GetCallerIdentity
        | O::HeadBucket
        | O::GetInstanceProfile
        | O::DescribeKey
        | O::DescribeRegions
        | O::DescribeAvailabilityZones
        | O::DescribeVpcAttribute
        | O::DescribeInstanceAttribute => Pagination::Singleton,
        _ => Pagination::Token,
    })
}

pub(super) fn round_failure(round: &ObservationRound, fallback: ReadFailureV1) -> ReadFailureV1 {
    match round.failure() {
        Some(LimitKind::Session) => ReadFailureV1::SessionExpired,
        Some(limit) => ReadFailureV1::Limit(limit),
        None => fallback,
    }
}

/// Bounded in-memory result; never a published object or admission capability.
#[derive(Debug)]
pub(super) struct QueryResult {
    pub records: Vec<ObservationEntryV5>,
    pub coverage: ReadCoverageV1,
}

pub(super) struct LogicalQuery {
    round: ObservationRound,
    accounting: QueryAccounting,
    pagination: Pagination,
    request_start: u64,
    page_start: Option<u64>,
    records: Vec<ObservationEntryV5>,
    occurrences: u64,
    tokens: BTreeSet<String>,
    next: Option<ContinuationToken>,
    terminal: bool,
    failure: Option<ReadFailureV1>,
    required: bool,
    reservation: usize,
    finished: bool,
    allocation_stamp: Option<Arc<()>>,
    source_credit: SourceCreditV5,
    legacy_minimum: u64,
}
impl LogicalQuery {
    pub(super) fn begin(
        round: ObservationRound,
        query: QueryIdentityV1,
        required: bool,
    ) -> ReadResult<Self> {
        let pagination = pagination(&query)?;
        // Enumerate the closed failure vocabulary, avoiding a guessed byte allowance.
        let mut reasons = vec![
            ReadFailureV1::AccessDenied,
            ReadFailureV1::NotFound,
            ReadFailureV1::Service,
            ReadFailureV1::Transport,
            ReadFailureV1::SessionExpired,
            ReadFailureV1::Unsupported,
            ReadFailureV1::Malformed,
            ReadFailureV1::PaginationCycle,
        ];
        reasons.extend(
            [
                LimitKind::Pages,
                LimitKind::Requests,
                LimitKind::Records,
                LimitKind::ResponseBytes,
                LimitKind::RoundResponseBytes,
                LimitKind::NormalizedBytes,
                LimitKind::RecordBytes,
                LimitKind::Elapsed,
                LimitKind::Clock,
                LimitKind::Session,
                LimitKind::Cancelled,
                LimitKind::Body,
            ]
            .into_iter()
            .map(ReadFailureV1::Limit),
        );
        let reserve = reasons
            .into_iter()
            .map(|reason| ReadCoverageV1 {
                query: query.clone(),
                required,
                requests: REQUESTS,
                pages: PAGES,
                records: RECORDS,
                terminal_page: false,
                status: CoverageStatus::Incomplete(reason),
            })
            .max_by_key(|c| {
                canonical::encode(c)
                    .expect("closed coverage encoding")
                    .len()
            })
            .expect("nonempty failure vocabulary");
        let reservation = round
            .begin_query(&reserve)
            .map_err(|_| round_failure(&round, ReadFailureV1::Unsupported))?;
        Ok(Self {
            request_start: round.requests(),
            round,
            accounting: QueryAccounting::new(query).expect("validated query"),
            pagination,
            page_start: None,
            records: Vec::new(),
            occurrences: 0,
            tokens: BTreeSet::new(),
            next: None,
            terminal: false,
            failure: None,
            required,
            reservation,
            finished: false,
            allocation_stamp: None,
            source_credit: SourceCreditV5::default(),
            legacy_minimum: 0,
        })
    }
    pub(super) fn query(&self) -> &QueryIdentityV1 {
        self.accounting.query()
    }
    pub(super) fn start_allocation_page(
        &mut self,
    ) -> ReadResult<(AllocationAttempt, Option<String>)> {
        let token = self.start_page()?.map(str::to_owned);
        let stamp = Arc::new(());
        self.allocation_stamp = Some(stamp.clone());
        Ok((
            AllocationAttempt {
                stamp,
                operation: self.query().operation,
            },
            token,
        ))
    }
    /// The callback projects already-qualified SDK data; it cannot dispatch requests.
    pub(super) fn allocation_page(
        &mut self,
        receipt: QualifiedAllocationReceipt,
        project: impl FnOnce(&mut AllocationPageSink<'_>) -> ReadResult<()>,
    ) -> ReadResult<()> {
        let mut pending = receipt
            .continuation()
            .filter(|s| s.len() > TOKEN_BYTES)
            .map(|_| ReadFailureV1::Limit(LimitKind::RecordBytes));
        let result = (|| {
            if let Some(reason) = self.failure {
                return Err(reason);
            }
            let stamp = self
                .allocation_stamp
                .take()
                .ok_or(ReadFailureV1::Malformed)?;
            if !receipt.matches(&stamp) {
                return Err(ReadFailureV1::Malformed);
            }
            self.account_attempt()?;
            self.terminal = receipt.continuation().is_none();
            self.round
                .records(receipt.occurrences())
                .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
            self.occurrences += receipt.occurrences();
            let page = self
                .accounting
                .attempted_pages()
                .try_into()
                .map_err(|_| ReadFailureV1::Malformed)?;
            let mut sink = AllocationPageSink {
                query: self,
                receipt: &receipt,
                page,
                pending: &mut pending,
            };
            project(&mut sink)?;
            if let Some(value) = receipt.continuation() {
                if value.len() > TOKEN_BYTES {
                    return Err(ReadFailureV1::Limit(LimitKind::RecordBytes));
                }
                if self.pagination == Pagination::Singleton {
                    return Err(ReadFailureV1::Malformed);
                }
                let token = ContinuationToken::parse(value.to_owned())
                    .map_err(|_| ReadFailureV1::Malformed)?;
                if !self.tokens.insert(token.as_str().to_owned()) {
                    return Err(ReadFailureV1::PaginationCycle);
                }
                self.next = Some(token);
            } else {
                self.next = None;
            }
            if let Some(reason) = pending {
                return Err(reason);
            }
            self.round
                .remaining()
                .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
            Ok(())
        })();
        if let Some(ReadFailureV1::Limit(limit)) = pending {
            self.round.fail(limit);
        }
        let result = if self.round.failure().is_some() {
            Err(round_failure(&self.round, ReadFailureV1::Malformed))
        } else {
            result
        };
        if let Err(reason) = result {
            self.failure.get_or_insert(reason);
        }
        result
    }
    /// No request is charged here: only a polled connector call charges a request/page attempt.
    pub(super) fn start_page(&mut self) -> ReadResult<Option<&str>> {
        if self.page_start.is_some() || self.terminal || self.failure.is_some() {
            return Err(*self.failure.get_or_insert(ReadFailureV1::Malformed));
        }
        if self.accounting.attempted_pages() == PAGES {
            self.round.fail(LimitKind::Pages);
            self.failure = Some(ReadFailureV1::Limit(LimitKind::Pages));
            return Err(self.failure.unwrap());
        }
        if self.round.remaining().is_err() {
            let reason = round_failure(&self.round, ReadFailureV1::Transport);
            self.failure = Some(reason);
            return Err(reason);
        }
        self.page_start = Some(self.round.requests());
        Ok(self.next.as_ref().map(ContinuationToken::as_str))
    }
    fn account_attempt(&mut self) -> ReadResult<()> {
        let start = self.page_start.take().ok_or(ReadFailureV1::Malformed)?;
        match self.round.requests().checked_sub(start) {
            Some(1) => self
                .accounting
                .attempt_page()
                .map_err(|_| ReadFailureV1::Limit(LimitKind::Pages)),
            Some(0) => Err(round_failure(&self.round, ReadFailureV1::Malformed)),
            _ => Err(ReadFailureV1::Malformed),
        }
    }
    /// Charge every decoded occurrence before checking/retaining normalized records.
    /// Page records arrive only from private adapters; this API cannot dispatch a request.
    pub(super) fn page(
        &mut self,
        records: Vec<ObservationEntryV5>,
        occurrences: u64,
        continuation: Option<String>,
        incomplete: Option<ReadFailureV1>,
    ) -> ReadResult<()> {
        // Retain bounded evidence before latching a normalization/continuation bound.
        // Every exit (including provenance or token syntax failure) must still latch it.
        let limit = match incomplete {
            Some(ReadFailureV1::Limit(limit)) => Some(limit),
            _ if continuation.as_ref().is_some_and(|v| v.len() > TOKEN_BYTES) => {
                Some(LimitKind::RecordBytes)
            }
            _ => None,
        };
        let result = match self.failure {
            Some(reason) => Err(reason),
            None => self.accept_page(records, occurrences, continuation, incomplete),
        };
        if let Some(limit) = limit {
            self.round.fail(limit);
        }
        let result = if self.round.failure().is_some() {
            Err(round_failure(&self.round, ReadFailureV1::Malformed))
        } else {
            result
        };
        if let Err(reason) = result {
            self.failure.get_or_insert(reason);
        }
        result
    }
    fn accept_page(
        &mut self,
        records: Vec<ObservationEntryV5>,
        occurrences: u64,
        continuation: Option<String>,
        incomplete: Option<ReadFailureV1>,
    ) -> ReadResult<()> {
        self.account_attempt()?;
        // A decoded terminal response remains terminal even if normalization exhausts a bound.
        // Completion additionally requires successful normalization and final time checks.
        self.terminal = continuation.is_none();
        self.retain_records(records, occurrences)?;
        match continuation {
            None => {
                self.terminal = true;
                self.next = None;
            }
            Some(value) => {
                if value.len() > TOKEN_BYTES {
                    return Err(ReadFailureV1::Limit(LimitKind::RecordBytes));
                }
                if self.pagination == Pagination::Singleton {
                    return Err(ReadFailureV1::Malformed);
                }
                let token =
                    ContinuationToken::parse(value).map_err(|_| ReadFailureV1::Malformed)?;
                if !self.tokens.insert(token.as_str().to_owned()) {
                    return Err(ReadFailureV1::PaginationCycle);
                }
                self.next = Some(token);
            }
        }
        if let Some(reason) = incomplete {
            return Err(reason);
        }
        self.round
            .remaining()
            .map_err(|_| round_failure(&self.round, ReadFailureV1::Transport))?;
        Ok(())
    }
    fn retain_records(
        &mut self,
        records: Vec<ObservationEntryV5>,
        occurrences: u64,
    ) -> ReadResult<()> {
        self.round
            .records(occurrences)
            .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
        self.occurrences += occurrences;
        let mut minimum = 0;
        for record in &records {
            if record.query() != self.query() {
                return Err(ReadFailureV1::Malformed);
            }
            minimum += match record {
                ObservationEntryV5::V2(_) => 1,
                ObservationEntryV5::V3(r) => r.data.minimum_occurrences(),
                ObservationEntryV5::V4(r) => r
                    .data
                    .minimum_occurrences()
                    .map_err(|_| ReadFailureV1::Malformed)?,
                // V5 can enter only through its consumed, integrity-qualified page path.
                ObservationEntryV5::V5(_) => return Err(ReadFailureV1::Malformed),
            };
        }
        if minimum > occurrences {
            return Err(ReadFailureV1::Malformed);
        }
        self.legacy_minimum += minimum;
        for record in records {
            self.round
                .output_slot()
                .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
            let encoded = match &record {
                ObservationEntryV5::V2(v) => self.round.canonical_record_v2(v),
                ObservationEntryV5::V3(v) => self.round.canonical_record_v3(v),
                ObservationEntryV5::V4(v) => self.round.canonical_record_v4(v),
                ObservationEntryV5::V5(_) => return Err(ReadFailureV1::Malformed),
            };
            encoded.map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
            self.round
                .retain_output()
                .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
            self.records.push(record);
        }
        Ok(())
    }
    pub(super) fn failed_page(&mut self, reason: ReadFailureV1) {
        self.failed_page_with_evidence(vec![], 0, reason);
    }
    /// A failed invocation can return independent facts, but never a successful terminal page.
    /// It uses the same occurrence/canonical charges and reserved coverage as a decoded page.
    pub(super) fn failed_page_with_evidence(
        &mut self,
        records: Vec<ObservationEntryV5>,
        occurrences: u64,
        reason: ReadFailureV1,
    ) {
        match reason {
            ReadFailureV1::SessionExpired => self.round.fail(LimitKind::Session),
            ReadFailureV1::Limit(limit) => self.round.fail(limit),
            _ => (),
        }
        let attempt = self
            .account_attempt()
            .and_then(|()| self.retain_records(records, occurrences));
        self.failure.get_or_insert(round_failure(
            &self.round,
            if self.round.requests() > self.request_start {
                attempt.err().unwrap_or(reason)
            } else {
                reason
            },
        ));
    }
    pub(super) fn finish(mut self) -> QueryResult {
        if self.page_start.is_some() {
            self.failed_page(ReadFailureV1::Malformed);
        }
        if self.round.remaining().is_err() {
            self.failure = Some(round_failure(&self.round, ReadFailureV1::Transport));
        }
        let status = match self.failure {
            Some(reason) => CoverageStatus::Incomplete(reason),
            None if self.terminal => CoverageStatus::Complete,
            None => CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        };
        let coverage = ReadCoverageV1 {
            query: self.query().clone(),
            required: self.required,
            requests: self.round.requests() - self.request_start,
            pages: self.accounting.attempted_pages(),
            records: self.occurrences,
            terminal_page: self.terminal,
            status,
        };
        // Reserved before any I/O, usable after a latch without resetting or bypassing accounting.
        assert!(
            canonical::encode(&coverage)
                .expect("coverage encoding")
                .len()
                <= self.reservation
        );
        self.finished = true;
        self.round.end_query();
        QueryResult {
            records: std::mem::take(&mut self.records),
            coverage,
        }
    }
}

pub(super) struct AllocationPageSink<'a> {
    query: &'a mut LogicalQuery,
    receipt: &'a QualifiedAllocationReceipt,
    page: SourcePageV5,
    pending: &'a mut Option<ReadFailureV1>,
}
impl AllocationPageSink<'_> {
    /// Preflight all owned facts before incremental retention can encounter an early exit.
    pub(super) fn inspect(&mut self, data: &ObservationDataV5) -> ReadResult<()> {
        let attribute = match self.query.query().scope {
            QueryScopeV1::InstanceAttribute { attribute, .. } => Some(attribute),
            _ => None,
        };
        let facts = data.facts().map_err(|_| ReadFailureV1::Malformed)?;
        *self.pending = combine_failure(
            *self.pending,
            combine_failure(facts.failure, data.required_failure(attribute)),
        );
        Ok(())
    }
    pub(super) fn note_failure(&mut self, reason: ReadFailureV1) {
        *self.pending = combine_failure(*self.pending, Some(reason));
    }
    pub(super) fn retain(&mut self, path: SourcePathV5, data: ObservationDataV5) -> ReadResult<()> {
        self.query
            .round
            .output_slot()
            .map_err(|_| round_failure(&self.query.round, ReadFailureV1::Malformed))?;
        self.receipt
            .validate_source(path, &data)
            .map_err(|_| ReadFailureV1::Malformed)?;
        let record = ObservationRecordV5 {
            schema_version: 5,
            query: self.query.query().clone(),
            source: SourceOccurrenceV5 {
                page: self.page,
                path,
            },
            data,
        };
        record.validate().map_err(|_| ReadFailureV1::Malformed)?;
        self.query
            .source_credit
            .record(
                record.source,
                record.data.projection(),
                record.data.claims(),
            )
            .map_err(|_| ReadFailureV1::Malformed)?;
        let minimum = self
            .query
            .source_credit
            .minimum()
            .map_err(|_| ReadFailureV1::Malformed)?;
        if minimum
            .checked_add(self.query.legacy_minimum)
            .is_none_or(|n| n > self.query.occurrences)
        {
            return Err(ReadFailureV1::Malformed);
        }
        self.query
            .round
            .canonical_record_v5(&record)
            .map_err(|_| round_failure(&self.query.round, ReadFailureV1::Malformed))?;
        self.query
            .round
            .retain_output()
            .map_err(|_| round_failure(&self.query.round, ReadFailureV1::Malformed))?;
        self.query
            .records
            .push(ObservationEntryV5::V5(Box::new(record)));
        Ok(())
    }
}
impl Drop for LogicalQuery {
    fn drop(&mut self) {
        if !self.finished {
            self.round.fail(LimitKind::Cancelled);
            self.round.end_query();
        }
    }
}

#[cfg(test)]
#[path = "query_execution_tests.rs"]
mod tests;
