//! A07 only: the reviewed subnet's explicit route-table associations.
use super::{
    ec2_decode_integrity::capture_ec2, ec2_observation::normalize, query_execution::round_failure,
    query_execution_core::QueryCore, response_limits::ObservationRound,
};
use crate::{
    Result,
    deployment::DeploymentV2,
    provider::{
        coverage::*, ec2_observation_v4::FactsV4, limits::*, manifest::ReviewedInfrastructureV1,
        reviewed_subnet_routes_v1::*,
    },
};

pub(super) struct RouteQueryResult {
    pub records: Vec<ReviewedSubnetRouteRecordV1>,
    pub coverage: ReviewedSubnetRouteCoverageV1,
}
pub(super) struct ReviewedSubnetRoutes {
    query: ReviewedSubnetRouteQueryV1,
    round: ObservationRound,
    ec2: aws_sdk_ec2::Client,
}
impl ReviewedSubnetRoutes {
    pub(super) fn new(
        deployment: &DeploymentV2,
        manifest: &[u8],
        conf: &aws_types::SdkConfig,
        round: ObservationRound,
    ) -> Result<Self> {
        let manifest = ReviewedInfrastructureV1::parse_bound(manifest, deployment)?;
        Ok(Self {
            query: ReviewedSubnetRouteQueryV1 {
                schema_version: 1,
                operation: ReadOperationV1::DescribeRouteTables,
                account: manifest.account,
                region: manifest.region,
                association_subnet: manifest.subnet,
            },
            round,
            ec2: aws_sdk_ec2::Client::from_conf(
                aws_sdk_ec2::config::Builder::from(conf)
                    .use_fips(false)
                    .use_dual_stack(false)
                    .build(),
            ),
        })
    }
    pub(super) fn query(&self) -> &ReviewedSubnetRouteQueryV1 {
        &self.query
    }
    pub(super) async fn observe(
        &mut self,
        required: bool,
    ) -> std::result::Result<RouteQueryResult, ReadFailureV1> {
        let mut core = QueryCore::begin(
            self.round.clone(),
            DiscoveryQuery::ReviewedSubnetRoutes(self.query.clone()),
            required,
        )?;
        let mut records = Vec::new();
        while let Ok(token) = core.start_page() {
            let token = token.map(str::to_owned);
            let (guard, receiver) =
                capture_ec2(ReadOperationV1::DescribeRouteTables, self.round.clone());
            let response = self
                .ec2
                .describe_route_tables()
                .filters(
                    aws_sdk_ec2::types::Filter::builder()
                        .name("association.subnet-id")
                        .values(self.query.association_subnet.as_str())
                        .build(),
                )
                .max_results(10)
                .set_next_token(token)
                .customize()
                .interceptor(guard)
                .send()
                .await;
            let response = response.map_err(|e| super::identity_reads::read_failure(&e));
            let result = (|| {
                core.account_attempt()?;
                response?;
                let (output, occurrences) =
                    receiver.take().map_err(|_| ReadFailureV1::Malformed)?;
                let page = normalize(output, occurrences, &self.round);
                core.terminal = page.returned_token.is_none();
                let pending = page.incomplete;
                let token_limit = page
                    .returned_token
                    .as_ref()
                    .is_some_and(|s| s.len() > TOKEN_BYTES);
                let accept = (|| {
                    self.round
                        .records(page.occurrences)
                        .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
                    core.occurrences += page.occurrences;
                    let mut minimum = 0;
                    for data in page.data {
                        minimum += data
                            .facts()
                            .map_err(|_| ReadFailureV1::Malformed)?
                            .occurrences;
                        if minimum > page.occurrences {
                            return Err(ReadFailureV1::Malformed);
                        }
                        self.round
                            .output_slot()
                            .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
                        let record = ReviewedSubnetRouteRecordV1 {
                            schema_version: 1,
                            query: self.query.clone(),
                            data,
                        };
                        self.round
                            .canonical_route_record(&record)
                            .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
                        self.round
                            .retain_output()
                            .map_err(|_| round_failure(&self.round, ReadFailureV1::Malformed))?;
                        records.push(record);
                    }
                    core.continuation(page.returned_token)?;
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
                } else if token_limit {
                    self.round.fail(LimitKind::RecordBytes);
                }
                accept
            })();
            if let Err(reason) = result {
                match reason {
                    ReadFailureV1::SessionExpired => self.round.fail(LimitKind::Session),
                    ReadFailureV1::Limit(limit) => self.round.fail(limit),
                    _ => (),
                }
                core.failure = Some(round_failure(&self.round, reason));
                break;
            }
            if core.terminal {
                break;
            }
        }
        let status = core.finish();
        let coverage = ReviewedSubnetRouteCoverageV1 {
            schema_version: 1,
            query: self.query.clone(),
            required,
            requests: self.round.requests() - core.request_start,
            pages: core.pages,
            records: core.occurrences,
            terminal_page: core.terminal,
            status,
        };
        assert!(
            crate::canonical::encode(&coverage)
                .expect("closed route coverage")
                .len()
                <= core.reservation
        );
        Ok(RouteQueryResult { records, coverage })
    }
}
