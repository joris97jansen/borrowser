//! Local AWS launch authority, never transport. Replayed receipts are facts, not capabilities.
use crate::{
    Error, Result, canonical,
    deployment::{AuthorityRootV2, DeploymentV2},
    identity::*,
    launch::{ClientToken, LaunchApprovalV2, LaunchSpecV2, RunInstancesRequestV2},
    model::{EnvelopeV2, EventV2},
    require,
    scheduling::{LaunchDispatchDeadline, TimeSample},
    trust::IdentityTrustV2,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchBindingV2 {
    pub authority_id: AuthorityId,
    pub account_id: AwsAccountId,
    pub region: Region,
    pub availability_zone_id: AvailabilityZoneId,
    pub operation_id: OperationId,
    pub deployment_sha256: DeploymentDigest,
    pub approval_sha256: LaunchApprovalDigest,
    pub trust_sha256: IdentityTrustDigest,
    pub spec_sha256: LaunchSpecDigest,
    pub request_sha256: LaunchRequestDigest,
    pub client_token: ClientToken,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchAuthorizationV2 {
    pub format: String,
    pub schema_version: u64,
    pub binding: LaunchBindingV2,
    pub expected_head: EventDigest,
    pub reviewer: ReviewText,
    pub reference: ReviewText,
    pub rationale: ReviewText,
    /// Human audit metadata only; operational timing comes from the journal.
    pub authorized_at_unix_seconds: u64,
}
impl LaunchAuthorizationV2 {
    pub fn digest(&self) -> Result<LaunchAuthorizationDigest> {
        require(
            self.format == "borrowser-aws-ec2-launch-authorization" && self.schema_version == 2,
            "launch authorization generation",
        )?;
        require(
            self.authorized_at_unix_seconds > 0
                && self.authorized_at_unix_seconds <= 253_402_300_799,
            "authorization audit time",
        )?;
        canonical::sha256(&canonical::encode(self)?).parse()
    }
}
/// In-memory inputs reconstructed from six independently retained canonical artifacts.
/// This aggregate is never a journal payload or a separately encoded artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedLaunchV2 {
    pub deployment: DeploymentV2,
    pub approval: LaunchApprovalV2,
    pub trust: IdentityTrustV2,
    pub specification: LaunchSpecV2,
    pub request: RunInstancesRequestV2,
    pub authorization: LaunchAuthorizationV2,
}
impl PreparedLaunchV2 {
    pub fn binding(&self) -> Result<LaunchBindingV2> {
        self.specification
            .validate(&self.deployment, &self.approval, &self.trust)?;
        self.request
            .validate(&self.deployment, &self.approval, &self.trust)?;
        require(
            self.request.spec == self.specification,
            "prepared request/spec binding",
        )?;
        Ok(LaunchBindingV2 {
            authority_id: self.specification.authority_id.clone(),
            account_id: self.specification.launch.account_id.clone(),
            region: self.specification.launch.region.clone(),
            availability_zone_id: self.specification.launch.availability_zone_id.clone(),
            operation_id: self.specification.operation_id.clone(),
            deployment_sha256: self.deployment.digest()?,
            approval_sha256: self.approval.digest(&self.deployment, &self.trust)?,
            trust_sha256: self.trust.digest()?,
            spec_sha256: self.specification.fingerprint()?,
            request_sha256: self.request.fingerprint(
                &self.deployment,
                &self.approval,
                &self.trust,
            )?,
            client_token: self.request.client_token.clone(),
        })
    }
    pub fn validate(
        &self,
        root: &AuthorityRootV2,
        head: &EventDigest,
        now: &TimeSample,
    ) -> Result<()> {
        require(
            self.deployment.marker()? == *root,
            "prepared authority root",
        )?;
        require(
            self.authorization.binding == self.binding()?
                && self.authorization.expected_head == *head,
            "authorization request/head binding",
        )?;
        self.authorization.digest()?;
        now.validate()?;
        LaunchDispatchDeadline::after(now)?;
        // Controller realtime checks review admission; it never sets retry delays.
        let seconds = now.realtime_ns / 1_000_000_000;
        require(
            self.authorization.authorized_at_unix_seconds <= seconds,
            "future authorization audit time",
        )?;
        self.approval.review.validate_at(seconds)?;
        self.approval
            .review
            .validate_at(self.authorization.authorized_at_unix_seconds)?;
        self.references()?;
        Ok(())
    }
}
/// Digest type parameters prevent cross-resource substitution in Rust. Field names
/// fix each expected artifact type on the wire; resolving bytes still requires
/// exact length/hash, canonical decoding and cross-binding validation.
/// ```compile_fail
/// use borrowser_host_lifecycle::{dispatch::RetainedArtifactV2, identity::{DeploymentDigest, IdentityTrustDigest}};
/// fn confuse(reference: RetainedArtifactV2<DeploymentDigest>) -> RetainedArtifactV2<IdentityTrustDigest> { reference }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedArtifactV2<D> {
    pub sha256: D,
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchArtifactRefsV2 {
    pub deployment: RetainedArtifactV2<DeploymentDigest>,
    pub approval: RetainedArtifactV2<LaunchApprovalDigest>,
    pub trust: RetainedArtifactV2<IdentityTrustDigest>,
    pub specification: RetainedArtifactV2<LaunchSpecDigest>,
    pub request: RetainedArtifactV2<LaunchRequestDigest>,
    pub authorization: RetainedArtifactV2<LaunchAuthorizationDigest>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchPreparationV2 {
    pub binding: LaunchBindingV2,
    pub artifacts: LaunchArtifactRefsV2,
}
impl PreparedLaunchV2 {
    pub fn references(&self) -> Result<LaunchArtifactRefsV2> {
        let binding = self.binding()?;
        Ok(LaunchArtifactRefsV2 {
            deployment: RetainedArtifactV2 {
                sha256: binding.deployment_sha256,
                bytes: canonical::encode(&self.deployment)?.len() as u64,
            },
            approval: RetainedArtifactV2 {
                sha256: binding.approval_sha256,
                bytes: canonical::encode(&self.approval)?.len() as u64,
            },
            trust: RetainedArtifactV2 {
                sha256: binding.trust_sha256,
                bytes: canonical::encode(&self.trust)?.len() as u64,
            },
            specification: RetainedArtifactV2 {
                sha256: binding.spec_sha256,
                bytes: canonical::encode(&self.specification)?.len() as u64,
            },
            request: RetainedArtifactV2 {
                sha256: binding.request_sha256,
                bytes: canonical::encode(&self.request)?.len() as u64,
            },
            authorization: RetainedArtifactV2 {
                sha256: self.authorization.digest()?,
                bytes: canonical::encode(&self.authorization)?.len() as u64,
            },
        })
    }
    pub fn preparation(&self) -> Result<LaunchPreparationV2> {
        Ok(LaunchPreparationV2 {
            binding: self.binding()?,
            artifacts: self.references()?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalReceiptV2 {
    pub sequence: u64,
    pub head: EventDigest,
}
impl JournalReceiptV2 {
    fn of(e: &EnvelopeV2) -> Result<Self> {
        Ok(Self {
            sequence: e.sequence,
            head: canonical::sha256(&canonical::encode(e)?).parse()?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchIntentV2 {
    pub binding: LaunchBindingV2,
    pub authorization_sha256: LaunchAuthorizationDigest,
    pub preparation: JournalReceiptV2,
    pub prepared_at: TimeSample,
    pub deadline: LaunchDispatchDeadline,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchIdentityV2 {
    pub intent: DispatchIntentV2,
    pub receipt: JournalReceiptV2,
    pub time: TimeSample,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct LaunchAttemptNumber(u64);
impl TryFrom<u64> for LaunchAttemptNumber {
    type Error = Error;
    fn try_from(value: u64) -> Result<Self> {
        require((1..=3).contains(&value), "launch attempt budget")?;
        Ok(Self(value))
    }
}
impl From<LaunchAttemptNumber> for u64 {
    fn from(value: LaunchAttemptNumber) -> Self {
        value.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptIntentV2 {
    pub dispatch: DispatchIdentityV2,
    pub number: LaunchAttemptNumber,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptIdentityV2 {
    pub intent: AttemptIntentV2,
    pub receipt: JournalReceiptV2,
    pub time: TimeSample,
}
/// Transport normalization contract only. No SDK error mapping in Pass 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchAttemptOutcome {
    DefinitelyNotTransmitted,
    TransmissionUncertain,
    ParameterConflict,
    AccessBlocked,
    ThrottledHeld,
    ResponseUnresolved,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptOutcomeV2 {
    pub attempt: AttemptIdentityV2,
    pub outcome: LaunchAttemptOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RetainedOutcomeV2 {
    pub outcome: LaunchAttemptOutcome,
    pub time: TimeSample,
    pub receipt: JournalReceiptV2,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AttemptRecordV2 {
    pub identity: AttemptIdentityV2,
    /// None is already uncertainty, including on replay; never a reusable permit.
    pub outcome: Option<RetainedOutcomeV2>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LaunchOperationV2 {
    pub prepared: PreparedLaunchV2,
    pub preparation: JournalReceiptV2,
    pub preparation_time: TimeSample,
    pub deadline: LaunchDispatchDeadline,
    pub dispatch: Option<DispatchIdentityV2>,
    pub attempts: Vec<AttemptRecordV2>,
}
impl LaunchOperationV2 {
    pub fn next_attempt(&self, now: &TimeSample) -> Result<AttemptIntentV2> {
        self.deadline.permits(&self.preparation_time, now)?;
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Error("dispatch intent required"))?;
        require(
            dispatch.time.same_clock(now) && now.boottime_ns >= dispatch.time.boottime_ns,
            "attempt before dispatch",
        )?;
        let number = LaunchAttemptNumber::try_from(self.attempts.len() as u64 + 1)?;
        if let Some(previous) = self.attempts.last() {
            let outcome = previous
                .outcome
                .as_ref()
                .ok_or(Error("unresolved attempt requires reconciliation"))?;
            // This exhaustive outcome gate must remain subordinate to any future
            // provider-ownership observations; response/uncertainty never retries.
            require(
                outcome.outcome == LaunchAttemptOutcome::DefinitelyNotTransmitted,
                "launch outcome blocks retry",
            )?;
            let delay = match previous.identity.intent.number.0 {
                1 => 2,
                2 => 8,
                _ => return Err(Error("launch attempt budget")),
            };
            crate::scheduling::retry_not_before(&outcome.time, delay, now)?;
        }
        Ok(AttemptIntentV2 {
            dispatch: dispatch.clone(),
            number,
        })
    }
}
/// Pure transactional reducer. The enclosing state installs this result only
/// after full envelope and event validation. There is no operation-close event.
pub(crate) fn reduce(
    current: &Option<LaunchOperationV2>,
    e: &EnvelopeV2,
    root: &AuthorityRootV2,
    retained: Option<&PreparedLaunchV2>,
) -> Result<Option<LaunchOperationV2>> {
    let mut next = current.clone();
    match &e.event {
        EventV2::LaunchPrepared(preparation) => {
            require(next.is_none(), "unresolved acquisition exists")?;
            let prepared = retained.ok_or(Error("retained launch artifacts required"))?;
            require(
                preparation == &prepared.preparation()?,
                "preparation retained artifact binding",
            )?;
            prepared.validate(
                root,
                e.previous_sha256
                    .as_ref()
                    .ok_or(Error("authorization requires head"))?,
                &e.time,
            )?;
            next = Some(LaunchOperationV2 {
                prepared: prepared.clone(),
                preparation: JournalReceiptV2::of(e)?,
                preparation_time: e.time.clone(),
                deadline: LaunchDispatchDeadline::after(&e.time)?,
                dispatch: None,
                attempts: Vec::new(),
            });
        }
        EventV2::LaunchDispatchIntent(intent) => {
            let op = next.as_mut().ok_or(Error("launch preparation required"))?;
            require(
                op.dispatch.is_none() && op.attempts.is_empty(),
                "single logical dispatch",
            )?;
            require(
                intent.binding == op.prepared.authorization.binding
                    && intent.authorization_sha256 == op.prepared.authorization.digest()?
                    && intent.preparation == op.preparation
                    && intent.prepared_at == op.preparation_time
                    && intent.deadline == op.deadline,
                "dispatch authorization binding",
            )?;
            op.deadline.permits(&op.preparation_time, &e.time)?;
            require(
                op.preparation_time.same_clock(&e.time)
                    && e.time.boottime_ns >= op.preparation_time.boottime_ns,
                "dispatch before preparation",
            )?;
            op.dispatch = Some(DispatchIdentityV2 {
                intent: intent.clone(),
                receipt: JournalReceiptV2::of(e)?,
                time: e.time.clone(),
            });
        }
        EventV2::LaunchAttemptIntent(intent) => {
            let op = next.as_mut().ok_or(Error("launch preparation required"))?;
            require(
                intent == &op.next_attempt(&e.time)?,
                "attempt dispatch binding/order",
            )?;
            op.attempts.push(AttemptRecordV2 {
                identity: AttemptIdentityV2 {
                    intent: intent.clone(),
                    receipt: JournalReceiptV2::of(e)?,
                    time: e.time.clone(),
                },
                outcome: None,
            });
        }
        EventV2::LaunchAttemptOutcome(outcome) => {
            let op = next.as_mut().ok_or(Error("launch preparation required"))?;
            let last = op.attempts.last_mut().ok_or(Error("attempt required"))?;
            require(
                last.outcome.is_none() && last.identity == outcome.attempt,
                "outcome exact pending attempt",
            )?;
            if last.identity.time.same_clock(&e.time) {
                require(
                    e.time.boottime_ns >= last.identity.time.boottime_ns,
                    "outcome time regression",
                )?;
            }
            last.outcome = Some(RetainedOutcomeV2 {
                outcome: outcome.outcome,
                time: e.time.clone(),
                receipt: JournalReceiptV2::of(e)?,
            });
        }
        EventV2::AuthorityInitialized => (),
        #[cfg(test)]
        EventV2::StorageCheckpoint | EventV2::StorageRecovery | EventV2::StorageEvidence { .. } => {
        }
    }
    Ok(next)
}

/// Not Clone, Serialize or Deserialize. Only durable publication constructs it.
/// ```compile_fail
/// use borrowser_host_lifecycle::dispatch::DurableLaunchAttempt;
/// let _: DurableLaunchAttempt = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::dispatch::DurableLaunchAttempt;
/// let _ = DurableLaunchAttempt { identity: todo!() };
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::dispatch::{DurableLaunchAttempt, AttemptIdentityV2};
/// fn forge(identity: AttemptIdentityV2) -> DurableLaunchAttempt { identity.into() }
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::dispatch::DurableLaunchAttempt;
/// fn retarget(attempt: &mut DurableLaunchAttempt) { attempt.identity().intent.number = 2.try_into().unwrap(); }
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::{dispatch::DurableLaunchAttempt, launch::RunInstancesRequestV2};
/// fn future_mutator(_: DurableLaunchAttempt) {}
/// fn substitute(request: RunInstancesRequestV2) { future_mutator(request); }
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::dispatch::DurableLaunchAttempt;
/// fn duplicate(attempt: DurableLaunchAttempt) { let _ = attempt.clone(); }
/// ```
#[derive(Debug)]
pub struct DurableLaunchAttempt {
    identity: AttemptIdentityV2,
}
impl DurableLaunchAttempt {
    pub fn identity(&self) -> &AttemptIdentityV2 {
        &self.identity
    }
}
/// A dispatch receipt is not itself permission to perform a wire attempt.
/// ```compile_fail
/// use borrowser_host_lifecycle::dispatch::DurableLaunchDispatch;
/// let _: DurableLaunchDispatch = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug)]
pub struct DurableLaunchDispatch {
    identity: DispatchIdentityV2,
}
impl DurableLaunchDispatch {
    pub fn identity(&self) -> &DispatchIdentityV2 {
        &self.identity
    }
}

// Internal plumbing only. The production CLI intentionally cannot call these.
#[cfg(unix)]
#[allow(dead_code)]
impl crate::journal::Journal {
    pub(crate) fn prepare_launch(
        &mut self,
        prepared: PreparedLaunchV2,
        time: TimeSample,
        tool: crate::model::ToolIdentityV2,
    ) -> Result<()> {
        self.check_storage_headroom()?;
        tool.validate()?;
        self.validate_launch_preparation(&prepared, &time)?;
        self.retain_launch_artifacts(&prepared)?;
        self.append_launch(EventV2::LaunchPrepared(prepared.preparation()?), time, tool)?;
        Ok(())
    }
    pub(crate) fn prepare_dispatch(
        &mut self,
        time: TimeSample,
        tool: crate::model::ToolIdentityV2,
    ) -> Result<DurableLaunchDispatch> {
        self.check_storage_headroom()?;
        self.verify_current_launch_artifacts()?;
        let op = self
            .state()
            .operation
            .as_ref()
            .ok_or(Error("launch preparation required"))?;
        let intent = DispatchIntentV2 {
            binding: op.prepared.authorization.binding.clone(),
            authorization_sha256: op.prepared.authorization.digest()?,
            preparation: op.preparation.clone(),
            prepared_at: op.preparation_time.clone(),
            deadline: op.deadline.clone(),
        };
        self.append_launch(EventV2::LaunchDispatchIntent(intent), time, tool)?;
        Ok(DurableLaunchDispatch {
            identity: self
                .state()
                .operation
                .as_ref()
                .and_then(|o| o.dispatch.clone())
                .ok_or(Error("durable dispatch missing"))?,
        })
    }
    pub(crate) fn begin_attempt(
        &mut self,
        time: TimeSample,
        tool: crate::model::ToolIdentityV2,
    ) -> Result<DurableLaunchAttempt> {
        self.check_storage_headroom()?;
        self.verify_current_launch_artifacts()?;
        let intent = self
            .state()
            .operation
            .as_ref()
            .ok_or(Error("launch preparation required"))?
            .next_attempt(&time)?;
        self.append_launch(EventV2::LaunchAttemptIntent(intent), time, tool)?;
        Ok(DurableLaunchAttempt {
            identity: self
                .state()
                .operation
                .as_ref()
                .and_then(|o| o.attempts.last())
                .ok_or(Error("durable attempt missing"))?
                .identity
                .clone(),
        })
    }
    pub(crate) fn finish_attempt(
        &mut self,
        attempt: DurableLaunchAttempt,
        outcome: LaunchAttemptOutcome,
        time: TimeSample,
        tool: crate::model::ToolIdentityV2,
    ) -> Result<()> {
        self.append_launch(
            EventV2::LaunchAttemptOutcome(AttemptOutcomeV2 {
                attempt: attempt.identity,
                outcome,
            }),
            time,
            tool,
        )?;
        Ok(())
    }
}
