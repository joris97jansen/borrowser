//! Immutable data only. Actual locked capture and fresh replay comparison belong to e3.
use super::{limits::ObservationLimitsV1, observation::EvidenceIdentityV1};
use crate::{
    Result, canonical, deployment::AuthorityRootV2, dispatch::*, identity::*, require,
    scheduling::TimeSample,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorProviderIdentityV1 {
    pub bound_instance: Option<InstanceId>,
    pub sticky_conflict: bool,
    /// Identity of the entire prior provider state, not merely the visible summary.
    pub state: ProviderStateDigest,
    pub evidence: Vec<EvidenceIdentityV1>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFieldsV1 {
    pub format: String,
    pub schema_version: u64,
    pub root: AuthorityRootV2,
    pub root_sha256: AuthorityRootDigest,
    pub head: EventDigest,
    pub next_sequence: u64,
    pub binding: LaunchBindingV2,
    pub artifacts: LaunchArtifactRefsV2,
    pub preparation: JournalReceiptV2,
    pub dispatch: Option<DispatchIdentityV2>,
    pub attempts: Vec<AttemptIdentityV2>,
    pub prior_provider: Option<PriorProviderIdentityV1>,
    pub manifest: InfrastructureDigest,
    pub evidence_policy_version: u64,
    pub limits: ObservationLimitsV1,
    pub captured_at: TimeSample,
}
/// Immutable owned fields contain no Journal, descriptor, client or launch capability.
/// This does not prove that a caller owns no OTHER authority handle. Network I/O
/// with any authority-lock ownership is explicitly invalid. E3 must drop all handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconciliationContextV1(ContextFieldsV1);
impl ReconciliationContextV1 {
    pub fn from_fields(fields: ContextFieldsV1) -> Result<Self> {
        require(
            fields.format == "borrowser-reconciliation-context"
                && fields.schema_version == 1
                && fields.evidence_policy_version == 1,
            "context version",
        )?;
        fields.limits.validate()?;
        fields.root.validate()?;
        require(
            fields.root.digest()? == fields.root_sha256,
            "context root digest",
        )?;
        let id = &fields.root.identity;
        require(
            fields.binding.authority_id == id.authority_id
                && fields.binding.account_id == id.account_id
                && fields.binding.region == id.region,
            "context authority binding",
        )?;
        let refs = &fields.artifacts;
        require(
            refs.deployment.sha256 == fields.binding.deployment_sha256
                && refs.approval.sha256 == fields.binding.approval_sha256
                && refs.trust.sha256 == fields.binding.trust_sha256
                && refs.specification.sha256 == fields.binding.spec_sha256
                && refs.request.sha256 == fields.binding.request_sha256,
            "context artifact binding",
        )?;
        for bytes in [
            refs.deployment.bytes,
            refs.approval.bytes,
            refs.trust.bytes,
            refs.specification.bytes,
            refs.request.bytes,
            refs.authorization.bytes,
        ] {
            require(
                (1..=canonical::EVENT_BYTES as u64).contains(&bytes),
                "context artifact length",
            )?;
        }
        require(
            fields.preparation.sequence < fields.next_sequence && fields.attempts.len() <= 3,
            "context receipt bounds",
        )?;
        if let Some(dispatch) = &fields.dispatch {
            require(
                dispatch.intent.binding == fields.binding
                    && dispatch.intent.preparation == fields.preparation
                    && dispatch.intent.authorization_sha256 == refs.authorization.sha256
                    && dispatch.receipt.sequence < fields.next_sequence,
                "context dispatch binding",
            )?;
            dispatch
                .intent
                .deadline
                .validate(&dispatch.intent.prepared_at)?;
        } else {
            require(fields.attempts.is_empty(), "attempt without dispatch")?;
        }
        for (i, attempt) in fields.attempts.iter().enumerate() {
            require(
                Some(&attempt.intent.dispatch) == fields.dispatch.as_ref()
                    && u64::from(attempt.intent.number) == i as u64 + 1
                    && attempt.receipt.sequence < fields.next_sequence,
                "context attempt binding",
            )?;
        }
        if let Some(prior) = &fields.prior_provider {
            require(prior.evidence.len() <= 128, "context prior evidence bound")?;
            super::canonical_set(&prior.evidence, 128)?;
            for e in &prior.evidence {
                e.validate()?;
            }
        }
        fields.captured_at.validate()?;
        require(
            canonical::encode(&fields)?.len() <= 32 << 10,
            "context canonical bound",
        )?;
        Ok(Self(fields))
    }
    pub fn fields(&self) -> &ContextFieldsV1 {
        &self.0
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        canonical::encode(&self.0)
    }
    pub fn identity(&self) -> Result<ReconciliationContextDigest> {
        canonical::sha256(&self.canonical_bytes()?).parse()
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() <= 32 << 10, "context byte bound")?;
        Self::from_fields(canonical::decode(bytes)?)
    }
}
