//! Exact non-secret Robot mutation bytes; descriptors alone grant no authority.
use crate::{Result, canonical, identity::*, provider::*, require};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationDescriptor {
    method: String,
    endpoint: String,
    body: String,
    body_sha256: RequestFingerprint,
}
impl MutationDescriptor {
    fn post(endpoint: String, body: String) -> Result<Self> {
        let body_sha256 = canonical::sha256(body.as_bytes()).parse()?;
        Ok(Self {
            method: "POST".into(),
            endpoint,
            body,
            body_sha256,
        })
    }
    pub fn allocation(request: &AllocationRequest) -> Result<Self> {
        Self::post("/order/server/transaction".into(), request.form()?)
    }
    pub fn cancellation(server: ServerNumber) -> Result<Self> {
        Self::post(
            format!("/server/{server}/cancellation"),
            CANCELLATION_FORM.into(),
        )
    }
    pub fn validate_allocation(&self, request: &AllocationRequest) -> Result<()> {
        require(
            *self == Self::allocation(request)?,
            "allocation descriptor mismatch",
        )
    }
    pub fn validate_cancellation(&self, server: ServerNumber) -> Result<()> {
        require(
            *self == Self::cancellation(server)?,
            "cancellation descriptor mismatch",
        )
    }
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn body(&self) -> &str {
        &self.body
    }
    pub fn body_sha256(&self) -> &RequestFingerprint {
        &self.body_sha256
    }
    pub fn fingerprint(&self) -> Result<RequestFingerprint> {
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationKind {
    Allocation,
    Cancellation,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchBinding {
    pub kind: MutationKind,
    pub authority: AuthorityId,
    pub account: AccountScopeId,
    pub operation: OperationId,
    /// Authority-wide immutable intent sequence identifies the one dispatch attempt.
    pub intent_sequence: u64,
    pub journal_head: EventDigest,
    pub descriptor: MutationDescriptor,
    pub descriptor_sha256: RequestFingerprint,
}
