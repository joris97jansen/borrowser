//! One private session composes closed identity and EC2 readers without scheduling or admission.
use super::{
    configuration,
    credentials::SessionSecret,
    ec2_allocation_reads::{AllocationObservations, AllocationRead},
    ec2_infrastructure_reads::{
        InfrastructureObservations, InfrastructureRead, InfrastructureTargets,
    },
    identity_reads::{IdentityObservations, IdentityRead},
    production_http,
    query_execution::QueryResult,
    response_limits::{BoundedHttp, ObservationRound},
};
use crate::{Result, deployment::DeploymentV2, provider::coverage::ReadFailureV1};
use std::{path::Path, time::SystemTime};
pub(super) struct ObservationSession {
    identity: IdentityObservations,
    infrastructure: InfrastructureObservations,
    allocation: AllocationObservations,
}
impl ObservationSession {
    pub(super) async fn allocation(
        &mut self,
        read: AllocationRead,
        required: bool,
    ) -> std::result::Result<QueryResult, ReadFailureV1> {
        self.allocation.observe(read, required).await
    }
    pub(super) async fn open(
        deployment: &DeploymentV2,
        manifest: &[u8],
        path: &Path,
    ) -> Result<Self> {
        let targets = InfrastructureTargets::parse(deployment, manifest)?;
        let round = ObservationRound::start()?;
        let secret = SessionSecret::load(path, SystemTime::now())?;
        Self::compose(deployment, targets, secret, production_http(round))
    }
    fn compose(
        deployment: &DeploymentV2,
        targets: InfrastructureTargets,
        secret: SessionSecret,
        http: BoundedHttp,
    ) -> Result<Self> {
        let round = http.round().clone();
        let conf = configuration(&secret, &deployment.identity.region, http)?;
        Ok(Self {
            identity: IdentityObservations::from_config(deployment, &conf, round.clone())?,
            infrastructure: InfrastructureObservations::from_config(targets, &conf, round.clone()),
            allocation: AllocationObservations::from_config(deployment, &conf, round),
        })
    }
    #[cfg(test)]
    pub(super) fn bounded(
        deployment: &DeploymentV2,
        manifest: &[u8],
        secret: SessionSecret,
        http: BoundedHttp,
    ) -> Result<Self> {
        let targets = InfrastructureTargets::parse(deployment, manifest)?;
        Self::compose(deployment, targets, secret, http)
    }
    pub(super) async fn identity(
        &mut self,
        read: IdentityRead,
        required: bool,
    ) -> std::result::Result<QueryResult, ReadFailureV1> {
        self.identity.observe(read, required).await
    }
    pub(super) async fn infrastructure(
        &mut self,
        read: InfrastructureRead,
        required: bool,
    ) -> std::result::Result<QueryResult, ReadFailureV1> {
        self.infrastructure.observe(read, required).await
    }
}
