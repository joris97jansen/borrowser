//! AWS generation 2: local authority and durable launch sequencing, no transport.
use crate::{
    Result, canonical, deployment::AuthorityRootV2, identity::*, require, scheduling::TimeSample,
};
use serde::{Deserialize, Serialize};
pub const FORMAT: &str = "borrowser-aws-ec2-lifecycle-event";
pub const AUTHORITY: &str = "aws-ec2-lifecycle-only";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolIdentityV2 {
    pub package: String,
    pub package_version: String,
    pub schema_version: u64,
    pub source_revision: String,
    pub cargo_lock_sha256: String,
    pub source_clean: bool,
}
impl ToolIdentityV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.package == "borrowser-host-lifecycle" && self.schema_version == 2,
            "tool generation",
        )?;
        canonical::text(&self.package_version, 64)?;
        require(self.source_clean, "unclean controller provenance")?;
        require(
            self.source_revision.len() == 40
                && self
                    .source_revision
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "source revision",
        )?;
        canonical::digest(&self.cargo_lock_sha256)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeV2 {
    pub account_id: AwsAccountId,
    pub authority: String,
    pub authority_id: AuthorityId,
    pub region: Region,
    pub root_sha256: AuthorityRootDigest,
    pub event: EventV2,
    pub format: String,
    pub previous_sha256: Option<EventDigest>,
    pub schema_version: u64,
    pub sequence: u64,
    pub time: TimeSample,
    pub tool: ToolIdentityV2,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum EventV2 {
    AuthorityInitialized,
    LaunchPrepared(crate::dispatch::LaunchPreparationV2),
    LaunchDispatchIntent(crate::dispatch::DispatchIntentV2),
    LaunchAttemptIntent(crate::dispatch::AttemptIntentV2),
    LaunchAttemptOutcome(crate::dispatch::AttemptOutcomeV2),
    // Storage fault tests exercise ordinary/recovery publication without adding
    // operational events or commands to the production generation.
    #[cfg(test)]
    StorageCheckpoint,
    #[cfg(test)]
    StorageRecovery,
    #[cfg(test)]
    StorageEvidence {
        sha256: String,
        bytes: u64,
    },
}
impl EventV2 {
    pub(crate) fn evidence(&self) -> Option<(&str, u64)> {
        match self {
            #[cfg(test)]
            Self::StorageEvidence { sha256, bytes } => Some((sha256, *bytes)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AuthorityStateV2 {
    pub sequence: u64,
    pub head: Option<EventDigest>,
    pub operation: Option<crate::dispatch::LaunchOperationV2>,
}
impl AuthorityStateV2 {
    pub fn apply(&self, e: &EnvelopeV2, root: &AuthorityRootV2) -> Result<Self> {
        self.apply_retained(e, root, None)
    }
    /// Pure application with resolved canonical artifacts. Journal callers must
    /// resolve these only from protected local storage, never external sources.
    pub fn apply_retained(
        &self,
        e: &EnvelopeV2,
        root: &AuthorityRootV2,
        retained: Option<&crate::dispatch::PreparedLaunchV2>,
    ) -> Result<Self> {
        root.validate()?;
        e.tool.validate()?;
        e.time.validate()?;
        require(
            e.format == FORMAT && e.authority == AUTHORITY && e.schema_version == 2,
            "envelope generation",
        )?;
        require(
            e.root_sha256 == root.digest()?
                && e.account_id == root.identity.account_id
                && e.authority_id == root.identity.authority_id
                && e.region == root.identity.region,
            "envelope root identity",
        )?;
        require(
            e.sequence == self.sequence && e.previous_sha256 == self.head,
            "journal chain",
        )?;
        if self.sequence == 0 {
            require(e.event == EventV2::AuthorityInitialized, "genesis required")?;
        } else {
            require(
                e.event != EventV2::AuthorityInitialized,
                "duplicate genesis",
            )?;
        }
        let operation = crate::dispatch::reduce(&self.operation, e, root, retained)?;
        Ok(Self {
            operation,
            sequence: self
                .sequence
                .checked_add(1)
                .ok_or(crate::Error("sequence overflow"))?,
            head: Some(canonical::sha256(&canonical::encode(e)?).parse()?),
        })
    }
}
impl EnvelopeV2 {
    #[cfg(any(test, target_os = "linux"))]
    pub(crate) fn genesis(
        root: &AuthorityRootV2,
        time: TimeSample,
        tool: ToolIdentityV2,
    ) -> Result<Self> {
        Ok(Self {
            account_id: root.identity.account_id.clone(),
            authority: AUTHORITY.into(),
            authority_id: root.identity.authority_id.clone(),
            region: root.identity.region.clone(),
            root_sha256: root.digest()?,
            event: EventV2::AuthorityInitialized,
            format: FORMAT.into(),
            previous_sha256: None,
            schema_version: 2,
            sequence: 0,
            time,
            tool,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn genesis_constructor_matches_independent_vector() {
        let fixture: EnvelopeV2 =
            canonical::decode(include_bytes!("../tests/fixtures/genesis-v2.json")).unwrap();
        let actual = EnvelopeV2::genesis(
            &crate::test_support::marker(),
            fixture.time.clone(),
            fixture.tool.clone(),
        )
        .unwrap();
        assert_eq!(actual, fixture);
    }
}
