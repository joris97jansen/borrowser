//! SDK-independent logical-query mechanics, private to the closed AWS boundary.
//! No clients, request closures, dispatch, discovery or policy live here.
use super::query_execution_core::QueryCore;
use super::response_limits::ObservationRound;
use super::{
    ec2_decode_integrity::allocation::QualifiedAllocationReceipt, ec2_observation::combine_failure,
};
use crate::provider::reviewed_subnet_routes_v1::DiscoveryQuery;
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
    core: QueryCore,
    records: Vec<ObservationEntryV5>,
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
        Ok(Self {
            core: QueryCore::begin(round, DiscoveryQuery::Existing(query), required)?,
            records: Vec::new(),
            allocation_stamp: None,
            source_credit: SourceCreditV5::default(),
            legacy_minimum: 0,
        })
    }
    pub(super) fn query(&self) -> &QueryIdentityV1 {
        match &self.core.identity {
            DiscoveryQuery::Existing(v) => v,
            _ => unreachable!("closed historical frontend"),
        }
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
            if let Some(reason) = self.core.failure {
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
            self.core.terminal = receipt.continuation().is_none();
            self.core
                .round
                .records(receipt.occurrences())
                .map_err(|_| round_failure(&self.core.round, ReadFailureV1::Malformed))?;
            self.core.occurrences += receipt.occurrences();
            let page = self
                .core
                .pages
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
                if self.core.pagination == Pagination::Singleton {
                    return Err(ReadFailureV1::Malformed);
                }
                let token = ContinuationToken::parse(value.to_owned())
                    .map_err(|_| ReadFailureV1::Malformed)?;
                if !self.core.tokens.insert(token.as_str().to_owned()) {
                    return Err(ReadFailureV1::PaginationCycle);
                }
                self.core.next = Some(token);
            } else {
                self.core.next = None;
            }
            if let Some(reason) = pending {
                return Err(reason);
            }
            self.core
                .round
                .remaining()
                .map_err(|_| round_failure(&self.core.round, ReadFailureV1::Malformed))?;
            Ok(())
        })();
        if let Some(ReadFailureV1::Limit(limit)) = pending {
            self.core.round.fail(limit);
        }
        let result = if self.core.round.failure().is_some() {
            Err(round_failure(&self.core.round, ReadFailureV1::Malformed))
        } else {
            result
        };
        if let Err(reason) = result {
            self.core.failure.get_or_insert(reason);
        }
        result
    }
    pub(super) fn start_page(&mut self) -> ReadResult<Option<&str>> {
        self.core.start_page()
    }
    fn account_attempt(&mut self) -> ReadResult<()> {
        self.core.account_attempt()
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
        let result = match self.core.failure {
            Some(reason) => Err(reason),
            None => self.accept_page(records, occurrences, continuation, incomplete),
        };
        if let Some(limit) = limit {
            self.core.round.fail(limit);
        }
        let result = if self.core.round.failure().is_some() {
            Err(round_failure(&self.core.round, ReadFailureV1::Malformed))
        } else {
            result
        };
        if let Err(reason) = result {
            self.core.failure.get_or_insert(reason);
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
        self.core.terminal = continuation.is_none();
        self.retain_records(records, occurrences)?;
        self.core.continuation(continuation)?;
        if let Some(reason) = incomplete {
            return Err(reason);
        }
        self.core
            .round
            .remaining()
            .map_err(|_| round_failure(&self.core.round, ReadFailureV1::Transport))?;
        Ok(())
    }
    fn retain_records(
        &mut self,
        records: Vec<ObservationEntryV5>,
        occurrences: u64,
    ) -> ReadResult<()> {
        self.core
            .round
            .records(occurrences)
            .map_err(|_| round_failure(&self.core.round, ReadFailureV1::Malformed))?;
        self.core.occurrences += occurrences;
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
            self.core
                .round
                .output_slot()
                .map_err(|_| round_failure(&self.core.round, ReadFailureV1::Malformed))?;
            let encoded = match &record {
                ObservationEntryV5::V2(v) => self.core.round.canonical_record_v2(v),
                ObservationEntryV5::V3(v) => self.core.round.canonical_record_v3(v),
                ObservationEntryV5::V4(v) => self.core.round.canonical_record_v4(v),
                ObservationEntryV5::V5(_) => return Err(ReadFailureV1::Malformed),
            };
            encoded.map_err(|_| round_failure(&self.core.round, ReadFailureV1::Malformed))?;
            self.core
                .round
                .retain_output()
                .map_err(|_| round_failure(&self.core.round, ReadFailureV1::Malformed))?;
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
            ReadFailureV1::SessionExpired => self.core.round.fail(LimitKind::Session),
            ReadFailureV1::Limit(limit) => self.core.round.fail(limit),
            _ => (),
        }
        let attempt = self
            .account_attempt()
            .and_then(|()| self.retain_records(records, occurrences));
        self.core.failure.get_or_insert(round_failure(
            &self.core.round,
            if self.core.round.requests() > self.core.request_start {
                attempt.err().unwrap_or(reason)
            } else {
                reason
            },
        ));
    }
    pub(super) fn finish(mut self) -> QueryResult {
        if self.core.page_start.is_some() {
            self.failed_page(ReadFailureV1::Malformed);
        }
        let status = self.core.finish();
        let coverage = ReadCoverageV1 {
            query: self.query().clone(),
            required: self.core.required,
            requests: self.core.round.requests() - self.core.request_start,
            pages: self.core.pages,
            records: self.core.occurrences,
            terminal_page: self.core.terminal,
            status,
        };
        // Reserved before any I/O, usable after a latch without resetting or bypassing accounting.
        assert!(
            canonical::encode(&coverage)
                .expect("coverage encoding")
                .len()
                <= self.core.reservation
        );
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
            .core
            .round
            .output_slot()
            .map_err(|_| round_failure(&self.query.core.round, ReadFailureV1::Malformed))?;
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
            .is_none_or(|n| n > self.query.core.occurrences)
        {
            return Err(ReadFailureV1::Malformed);
        }
        self.query
            .core
            .round
            .canonical_record_v5(&record)
            .map_err(|_| round_failure(&self.query.core.round, ReadFailureV1::Malformed))?;
        self.query
            .core
            .round
            .retain_output()
            .map_err(|_| round_failure(&self.query.core.round, ReadFailureV1::Malformed))?;
        self.query
            .records
            .push(ObservationEntryV5::V5(Box::new(record)));
        Ok(())
    }
}
#[cfg(test)]
#[path = "query_execution_tests.rs"]
mod tests;
