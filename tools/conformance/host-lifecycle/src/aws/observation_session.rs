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
    routes: super::reviewed_subnet_route_reads::ReviewedSubnetRoutes,
    round: ObservationRound,
    deployment: crate::identity::DeploymentDigest,
}
impl ObservationSession {
    pub(super) fn deployment(&self) -> &crate::identity::DeploymentDigest {
        &self.deployment
    }
    pub(super) fn round(&self) -> &ObservationRound {
        &self.round
    }
    pub(super) async fn reviewed_subnet_routes(
        &mut self,
    ) -> std::result::Result<super::reviewed_subnet_route_reads::RouteQueryResult, ReadFailureV1>
    {
        self.routes.observe(true).await
    }
    pub(super) async fn observe_query(
        &mut self,
        query: &crate::provider::coverage::QueryIdentityV1,
    ) -> std::result::Result<QueryResult, ReadFailureV1> {
        use crate::provider::coverage::{ReadOperationV1 as O, VpcAttribute};
        let identity = match query.operation {
            O::GetCallerIdentity => Some(IdentityRead::Caller),
            O::HeadBucket => Some(IdentityRead::Bucket),
            O::GetInstanceProfile => Some(IdentityRead::Profile),
            O::DescribeKey => Some(IdentityRead::Key),
            _ => None,
        };
        if let Some(read) = identity {
            if self.identity.query(read) != *query {
                return Err(ReadFailureV1::Unsupported);
            }
            return self.identity(read, true).await;
        }
        let reads = [
            InfrastructureRead::Regions,
            InfrastructureRead::AvailabilityZones,
            InfrastructureRead::Subnets,
            InfrastructureRead::Vpcs,
            InfrastructureRead::SecurityGroups,
            InfrastructureRead::RouteTables,
            InfrastructureRead::VpcEndpoints,
            InfrastructureRead::PrefixLists,
            InfrastructureRead::VpcAttribute(VpcAttribute::EnableDnsSupport),
            InfrastructureRead::VpcAttribute(VpcAttribute::EnableDnsHostnames),
            InfrastructureRead::DhcpOptions,
            InfrastructureRead::NetworkAcls,
        ];
        if let Some(read) = reads
            .into_iter()
            .find(|r| self.infrastructure.query(*r) == *query)
        {
            return self.infrastructure(read, true).await;
        }
        self.allocation(
            AllocationRead::try_from((query.operation, query.scope.clone()))?,
            true,
        )
        .await
    }
    pub(super) fn route_query(
        &self,
    ) -> &crate::provider::reviewed_subnet_routes_v1::ReviewedSubnetRouteQueryV1 {
        self.routes.query()
    }

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
        Self::compose(
            deployment,
            manifest,
            targets,
            secret,
            production_http(round),
        )
    }
    fn compose(
        deployment: &DeploymentV2,
        manifest: &[u8],
        targets: InfrastructureTargets,
        secret: SessionSecret,
        http: BoundedHttp,
    ) -> Result<Self> {
        let round = http.round().clone();
        let conf = configuration(&secret, &deployment.identity.region, http)?;
        Ok(Self {
            identity: IdentityObservations::from_config(deployment, &conf, round.clone())?,
            infrastructure: InfrastructureObservations::from_config(targets, &conf, round.clone()),
            allocation: AllocationObservations::from_config(deployment, &conf, round.clone()),
            routes: super::reviewed_subnet_route_reads::ReviewedSubnetRoutes::new(
                deployment,
                manifest,
                &conf,
                round.clone(),
            )?,
            round,
            deployment: deployment.digest()?,
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
        Self::compose(deployment, manifest, targets, secret, http)
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
