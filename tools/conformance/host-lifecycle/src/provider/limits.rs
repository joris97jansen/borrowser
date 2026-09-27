//! Representation limits are not a future durable envelope or packing scheme.
use crate::{Error, Result, canonical, require};
use serde::{Deserialize, Serialize};

pub const PAGES: u64 = 16;
pub const REQUESTS: u64 = 128;
pub const RECORDS: u64 = 4096;
pub const RESPONSE_BYTES: u64 = 1 << 20;
pub const ROUND_RESPONSE_BYTES: u64 = 8 << 20;
pub const NORMALIZED_BYTES: u64 = 256 << 10;
pub const RECORD_BYTES: usize = 16 << 10;
pub const FUTURE_OBJECT_BYTES: u64 = 32 << 10;
pub const OVERHEAD_BYTES: u64 = 256 << 10;
pub const PUBLICATION_BYTES: u64 = 512 << 10;
pub const PUBLICATION_OBJECTS: u64 = 35;
pub const OBSERVATION_NS: u64 = 300_000_000_000;
pub const TOKEN_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationLimitsV1 {
    pub schema_version: u64,
}
impl ObservationLimitsV1 {
    pub const V1: Self = Self { schema_version: 1 };
    pub fn validate(self) -> Result<()> {
        require(self == Self::V1, "observation limits version")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LimitKind {
    Pages,
    Requests,
    Records,
    ResponseBytes,
    RoundResponseBytes,
    NormalizedBytes,
    RecordBytes,
    Elapsed,
    Clock,
    Session,
    Cancelled,
    Body,
}

/// Counter for one immutable logical query; traversal and cycle detection belong to e2.
#[derive(Clone, Debug)]
pub struct QueryAccounting {
    query: super::coverage::QueryIdentityV1,
    attempted_pages: u64,
}
impl QueryAccounting {
    pub fn new(query: super::coverage::QueryIdentityV1) -> Result<Self> {
        query.validate()?;
        Ok(Self {
            query,
            attempted_pages: 0,
        })
    }
    pub fn query(&self) -> &super::coverage::QueryIdentityV1 {
        &self.query
    }
    pub fn attempted_pages(&self) -> u64 {
        self.attempted_pages
    }
    pub fn attempt_page(&mut self) -> Result<()> {
        require(self.attempted_pages < PAGES, "query page limit")?;
        self.attempted_pages += 1;
        Ok(())
    }
}
/// Opaque in-memory continuation, never serialized or exposed by Debug.
pub struct ContinuationToken(String);
impl ContinuationToken {
    pub fn parse(value: String) -> Result<Self> {
        require(
            !value.is_empty() && value.len() <= TOKEN_BYTES && !value.contains('\0'),
            "continuation token bound",
        )?;
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for ContinuationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ContinuationToken([redacted])")
    }
}

/// Latches exhaustion: no refund, reset or partial acceptance after failure.
#[derive(Clone, Debug, Default)]
pub struct ObservationAccounting {
    requests: u64,
    records: u64,
    response_bytes: u64,
    normalized_bytes: u64,
    failure: Option<LimitKind>,
}
impl ObservationAccounting {
    pub fn failure(&self) -> Option<LimitKind> {
        self.failure
    }
    pub fn requests(&self) -> u64 {
        self.requests
    }
    pub fn response_bytes(&self) -> u64 {
        self.response_bytes
    }
    pub fn fail(&mut self, reason: LimitKind) {
        self.failure.get_or_insert(reason);
    }
    fn active(&self) -> Result<()> {
        require(self.failure.is_none(), "observation budget exhausted")
    }
    pub fn request(&mut self) -> Result<()> {
        self.active()?;
        if self.requests >= REQUESTS {
            self.fail(LimitKind::Requests);
            return Err(Error("request limit"));
        }
        self.requests += 1;
        Ok(())
    }
    pub fn records(&mut self, count: u64) -> Result<()> {
        self.active()?;
        match self.records.checked_add(count).filter(|n| *n <= RECORDS) {
            Some(n) => {
                self.records = n;
                Ok(())
            }
            None => {
                self.fail(LimitKind::Records);
                Err(Error("record limit"))
            }
        }
    }
    /// Called BEFORE copying a frame. An offending frame is never charged as accepted.
    pub fn frame(&mut self, accepted_in_response: u64, bytes: u64) -> Result<()> {
        self.active()?;
        if accepted_in_response
            .checked_add(bytes)
            .is_none_or(|n| n > RESPONSE_BYTES)
        {
            self.fail(LimitKind::ResponseBytes);
            return Err(Error("response byte limit"));
        }
        match self
            .response_bytes
            .checked_add(bytes)
            .filter(|n| *n <= ROUND_RESPONSE_BYTES)
        {
            Some(n) => {
                self.response_bytes = n;
                Ok(())
            }
            None => {
                self.fail(LimitKind::RoundResponseBytes);
                Err(Error("round byte limit"))
            }
        }
    }
    /// Input has already passed its closed contract validator. This is NOT a durable envelope.
    pub fn canonical_record<T: Serialize>(&mut self, value: &T) -> Result<Vec<u8>> {
        self.active()?;
        let bytes = match canonical::encode(value) {
            Ok(b) if b.len() <= RECORD_BYTES => b,
            _ => {
                self.fail(LimitKind::RecordBytes);
                return Err(Error("canonical record limit"));
            }
        };
        match self
            .normalized_bytes
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= NORMALIZED_BYTES)
        {
            Some(n) => {
                self.normalized_bytes = n;
                Ok(bytes)
            }
            None => {
                self.fail(LimitKind::NormalizedBytes);
                Err(Error("normalized evidence limit"))
            }
        }
    }
}
