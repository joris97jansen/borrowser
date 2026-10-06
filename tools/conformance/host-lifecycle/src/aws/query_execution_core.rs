//! Shared bounded query mechanics for the two closed evidence frontends.
use super::{
    query_execution::{Pagination, round_failure},
    response_limits::ObservationRound,
};
use crate::provider::{coverage::*, limits::*, reviewed_subnet_routes_v1::DiscoveryQuery};
use std::collections::BTreeSet;
type ReadResult<T> = std::result::Result<T, ReadFailureV1>;

pub(super) struct QueryCore {
    pub round: ObservationRound,
    pub identity: DiscoveryQuery,
    pub pagination: Pagination,
    pub request_start: u64,
    pub page_start: Option<u64>,
    pub pages: u64,
    pub occurrences: u64,
    pub tokens: BTreeSet<String>,
    pub next: Option<ContinuationToken>,
    pub terminal: bool,
    pub failure: Option<ReadFailureV1>,
    pub required: bool,
    pub reservation: usize,
    pub finished: bool,
}
impl QueryCore {
    pub(super) fn begin(
        round: ObservationRound,
        identity: DiscoveryQuery,
        required: bool,
    ) -> ReadResult<Self> {
        let pagination = match &identity {
            DiscoveryQuery::Existing(v) => super::query_execution::pagination(v)?,
            DiscoveryQuery::ReviewedSubnetRoutes(v) => {
                v.validate().map_err(|_| ReadFailureV1::Malformed)?;
                Pagination::Token
            }
        };
        let reservation = identity
            .reservation(required)
            .map_err(|_| ReadFailureV1::Malformed)?;
        round
            .begin_discovery_query(&identity, reservation)
            .map_err(|_| round_failure(&round, ReadFailureV1::Unsupported))?;
        Ok(Self {
            request_start: round.requests(),
            round,
            identity,
            pagination,
            page_start: None,
            pages: 0,
            occurrences: 0,
            tokens: BTreeSet::new(),
            next: None,
            terminal: false,
            failure: None,
            required,
            reservation,
            finished: false,
        })
    }
    pub(super) fn start_page(&mut self) -> ReadResult<Option<&str>> {
        if self.page_start.is_some() || self.terminal || self.failure.is_some() {
            return Err(*self.failure.get_or_insert(ReadFailureV1::Malformed));
        }
        if self.pages == PAGES {
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
    pub(super) fn account_attempt(&mut self) -> ReadResult<()> {
        let start = self.page_start.take().ok_or(ReadFailureV1::Malformed)?;
        match self.round.requests().checked_sub(start) {
            Some(1) if self.pages < PAGES => {
                self.pages += 1;
                Ok(())
            }
            Some(0) => Err(round_failure(&self.round, ReadFailureV1::Malformed)),
            _ => Err(ReadFailureV1::Malformed),
        }
    }
    pub(super) fn continuation(&mut self, continuation: Option<String>) -> ReadResult<()> {
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
        Ok(())
    }
    pub(super) fn finish(&mut self) -> CoverageStatus {
        if self.round.remaining().is_err() {
            self.failure = Some(round_failure(&self.round, ReadFailureV1::Transport));
        }
        let status = match self.failure {
            Some(reason) => CoverageStatus::Incomplete(reason),
            None if self.terminal => CoverageStatus::Complete,
            None => CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        };
        self.finished = true;
        self.round.end_query();
        status
    }
}
impl Drop for QueryCore {
    fn drop(&mut self) {
        if !self.finished {
            self.round.fail(LimitKind::Cancelled);
            self.round.end_query();
        }
    }
}
