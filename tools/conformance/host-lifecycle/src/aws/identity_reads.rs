//! Four closed identity-service adapters. Observations never confer AwsSession admission.
use super::{
    configuration,
    credentials::SessionSecret,
    iam_presence::capture_instance_profile,
    identity_observation::normalize_key,
    production_http,
    query_execution::{LogicalQuery, QueryResult, round_failure},
    response_limits::{BoundedHttp, ObservationRound},
};
use crate::{
    Error, Result,
    deployment::DeploymentV2,
    identity::*,
    provider::{
        coverage::*,
        identity_observation_v3::*,
        inventory::ResourceIdentity,
        limits::LimitKind,
        management_observation_v2::ObservationValueV2,
        observation::{Observed, ProviderText},
        observation_v2::{ObservationDataV2, ObservationRecordV2},
        observation_v3::{ObservationDataV3, ObservationRecordV3},
        observation_v5::ObservationEntryV5,
    },
};
use aws_smithy_runtime_api::client::{orchestrator::HttpResponse, result::SdkError};
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use std::{path::Path, time::SystemTime};

#[derive(Clone, Copy)]
pub(super) enum IdentityRead {
    Caller,
    Bucket,
    Profile,
    Key,
}

/// Immutable reviewed inputs; no arbitrary query, endpoint, operation or SDK-client accessor.
/// A separate read boundary permits caller contradictions to be observed even when
/// the unchanged AwsSession admission path would reject them.
pub(super) struct IdentityObservations {
    account: AwsAccountId,
    region: Region,
    bucket: EvidenceBucketName,
    profile: InstanceProfileArn,
    profile_name: String,
    key: KmsKeyArn,
    round: ObservationRound,
    sts: aws_sdk_sts::Client,
    s3: aws_sdk_s3::Client,
    iam: aws_sdk_iam::Client,
    kms: aws_sdk_kms::Client,
}
impl IdentityObservations {
    pub(super) async fn open(deployment: &DeploymentV2, path: &Path) -> Result<Self> {
        deployment.support()?;
        let round = ObservationRound::start()?;
        let secret = SessionSecret::load(path, SystemTime::now())?;
        Self::bounded(deployment, secret, production_http(round))
    }
    fn bounded(
        deployment: &DeploymentV2,
        secret: SessionSecret,
        http: BoundedHttp,
    ) -> Result<Self> {
        deployment.support()?;
        let round = http.round().clone();
        let conf = configuration(&secret, &deployment.identity.region, http)?;
        Self::from_config(deployment, &conf, round)
    }
    pub(super) fn from_config(
        deployment: &DeploymentV2,
        conf: &aws_types::SdkConfig,
        round: ObservationRound,
    ) -> Result<Self> {
        let support = deployment.support()?;
        let profile_name = support
            .instance_profile_arn
            .rsplit('/')
            .next()
            .ok_or(Error("profile name"))?;
        crate::require(
            !profile_name.is_empty()
                && profile_name.len() <= 128
                && profile_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_+=,.@-".contains(&b)),
            "reviewed profile name",
        )?;
        Ok(Self {
            account: deployment.identity.account_id.clone(),
            region: deployment.identity.region.clone(),
            bucket: support.evidence_bucket.clone(),
            profile: support.instance_profile_arn.clone(),
            profile_name: profile_name.to_owned(),
            key: support.kms_key_arn.clone(),
            round,
            sts: aws_sdk_sts::Client::from_conf(aws_sdk_sts::config::Builder::from(conf).build()),
            s3: aws_sdk_s3::Client::from_conf(
                aws_sdk_s3::config::Builder::from(conf)
                    .disable_s3_express_session_auth(true)
                    .build(),
            ),
            iam: aws_sdk_iam::Client::from_conf(
                aws_sdk_iam::config::Builder::from(conf)
                    .use_fips(false)
                    .use_dual_stack(false)
                    .build(),
            ),
            kms: aws_sdk_kms::Client::from_conf(
                aws_sdk_kms::config::Builder::from(conf)
                    .use_fips(false)
                    .use_dual_stack(false)
                    .build(),
            ),
        })
    }
    pub(super) fn query(&self, read: IdentityRead) -> QueryIdentityV1 {
        let (operation, resource) = match read {
            IdentityRead::Caller => (ReadOperationV1::GetCallerIdentity, None),
            IdentityRead::Bucket => (
                ReadOperationV1::HeadBucket,
                Some(ResourceIdentity::Bucket(self.bucket.clone())),
            ),
            IdentityRead::Profile => (
                ReadOperationV1::GetInstanceProfile,
                Some(ResourceIdentity::Profile(self.profile.clone())),
            ),
            IdentityRead::Key => (
                ReadOperationV1::DescribeKey,
                Some(ResourceIdentity::Key(self.key.clone())),
            ),
        };
        QueryIdentityV1 {
            operation,
            account: self.account.clone(),
            region: self.region.clone(),
            scope: resource.map_or(QueryScopeV1::Regional, |id| QueryScopeV1::Exact {
                identities: vec![id],
            }),
        }
    }
    /// Serial mutable ownership plus a shared-round lease excludes overlapping queries.
    /// Err means no query could be admitted/reserved; no request was attempted.
    pub(super) async fn observe(
        &mut self,
        read: IdentityRead,
        required: bool,
    ) -> std::result::Result<QueryResult, ReadFailureV1> {
        let mut query = LogicalQuery::begin(self.round.clone(), self.query(read), required)?;
        if query.start_page().is_err() {
            return Ok(query.finish());
        }
        let result = match read {
            IdentityRead::Caller => self
                .sts
                .get_caller_identity()
                .send()
                .await
                .map(|o| {
                    let mut incomplete = None;
                    let data = ObservationDataV2::Caller {
                        account: typed(o.account(), &mut incomplete),
                        arn: text(o.arn(), &mut incomplete),
                        user_id: text(o.user_id(), &mut incomplete),
                    };
                    (
                        ObservationEntryV5::V2(Box::new(ObservationRecordV2 {
                            schema_version: 2,
                            query: query.query().clone(),
                            data,
                        })),
                        1,
                        incomplete,
                    )
                })
                .map_err(|e| read_failure(&e)),
            IdentityRead::Bucket => match self
                .s3
                .head_bucket()
                .bucket(self.bucket.as_str())
                .expected_bucket_owner(self.account.as_str())
                .send()
                .await
            {
                Ok(o) => {
                    let mut incomplete = None;
                    let data = ObservationDataV2::Bucket {
                        name: self.bucket.clone(),
                        expected_owner: self.account.clone(),
                        region: typed(o.bucket_region(), &mut incomplete),
                    };
                    Ok((
                        ObservationEntryV5::V2(Box::new(ObservationRecordV2 {
                            schema_version: 2,
                            query: query.query().clone(),
                            data,
                        })),
                        1,
                        incomplete,
                    ))
                }
                Err(e) => {
                    let reason = read_failure(&e);
                    // Only metadata attached to this failed invocation is eligible.
                    match e
                        .raw_response()
                        .map(|response| bucket_error_region(response, &self.round))
                        .transpose()
                    {
                        Ok(Some(Some(region))) => {
                            let record = ObservationEntryV5::V2(Box::new(ObservationRecordV2 {
                                schema_version: 2,
                                query: query.query().clone(),
                                data: ObservationDataV2::Bucket {
                                    name: self.bucket.clone(),
                                    expected_owner: self.account.clone(),
                                    region: Observed::Present(region),
                                },
                            }));
                            query.failed_page_with_evidence(vec![record], 1, reason);
                        }
                        Ok(_) => query.failed_page(reason),
                        Err(limit) => query.failed_page(ReadFailureV1::Limit(limit)),
                    }
                    return Ok(query.finish());
                }
            },
            IdentityRead::Profile => {
                let (hook, receiver) = capture_instance_profile(self.round.clone());
                match self
                    .iam
                    .get_instance_profile()
                    .instance_profile_name(&self.profile_name)
                    .customize()
                    .interceptor(hook)
                    .send()
                    .await
                {
                    Ok(_) => receiver
                        .take()
                        .map(|o| successor(query.query(), o))
                        .map_err(|_| ReadFailureV1::Malformed),
                    Err(e) => Err(read_failure(&e)),
                }
            }
            IdentityRead::Key => match self
                .kms
                .describe_key()
                .key_id(self.key.as_str())
                .send()
                .await
            {
                Ok(o) => normalize_key(&o)
                    .map(|o| successor(query.query(), o))
                    .map_err(|_| ReadFailureV1::Malformed),
                Err(e) => Err(read_failure(&e)),
            },
        };
        match result {
            Ok((record, occurrences, incomplete)) => {
                let _ = query.page(vec![record], occurrences, None, incomplete);
            }
            Err(reason) => query.failed_page(round_failure(&self.round, reason)),
        }
        Ok(query.finish())
    }
}

// Frozen V2 has no NotReturned or raw-malformed typed member state. Do not invent
// absence or change its encoding: mark the affected member unavailable and retain siblings.
fn typed<T: std::str::FromStr>(
    value: Option<&str>,
    incomplete: &mut Option<ReadFailureV1>,
) -> Observed<T> {
    match value.and_then(|v| v.parse().ok()) {
        Some(v) => Observed::Present(v),
        None => {
            incomplete.get_or_insert(ReadFailureV1::Malformed);
            Observed::Unavailable(ReadFailureV1::Malformed)
        }
    }
}
fn text(value: Option<&str>, incomplete: &mut Option<ReadFailureV1>) -> Observed<ProviderText> {
    match value {
        Some(v) => match v.to_owned().try_into() {
            Ok(v) => Observed::Present(v),
            Err(_) => {
                let reason = if v.len() > 2048 {
                    ReadFailureV1::Limit(LimitKind::RecordBytes)
                } else {
                    ReadFailureV1::Malformed
                };
                if incomplete.is_none() || matches!(reason, ReadFailureV1::Limit(_)) {
                    *incomplete = Some(reason);
                }
                Observed::Unavailable(reason)
            }
        },
        None => {
            incomplete.get_or_insert(ReadFailureV1::Malformed);
            Observed::Unavailable(ReadFailureV1::Malformed)
        }
    }
}
fn successor(
    query: &QueryIdentityV1,
    value: super::identity_observation::NormalizedIdentityV3,
) -> (ObservationEntryV5, u64, Option<ReadFailureV1>) {
    let incomplete = normalization_failure(&value.data);
    (
        ObservationEntryV5::V3(Box::new(ObservationRecordV3 {
            schema_version: 3,
            query: query.clone(),
            data: value.data,
        })),
        value.occurrences,
        incomplete,
    )
}
fn normalization_failure(data: &ObservationDataV3) -> Option<ReadFailureV1> {
    fn member<T>(v: &IdentityMemberV3<T>, failure: &mut Option<ReadFailureV1>) {
        let reason = match v {
            IdentityMemberV3::Malformed(_)
            | IdentityMemberV3::Unrepresentable(MemberRepresentationFailureV3::ContainsNul) => {
                ReadFailureV1::Malformed
            }
            IdentityMemberV3::Unrepresentable(MemberRepresentationFailureV3::TextBytes) => {
                ReadFailureV1::Limit(LimitKind::RecordBytes)
            }
            _ => return,
        };
        if failure.is_none() || matches!(reason, ReadFailureV1::Limit(_)) {
            *failure = Some(reason);
        }
    }
    let mut failure = None;
    match data {
        ObservationDataV3::Key {
            metadata: ObservationValueV2::Present(v),
        } => {
            member(&v.arn, &mut failure);
            member(&v.account, &mut failure);
            for v in [&v.manager, &v.spec, &v.usage, &v.state] {
                member(v, &mut failure);
            }
        }
        ObservationDataV3::Profile {
            profile: ObservationValueV2::Present(v),
        } => {
            member(&v.arn, &mut failure);
            member(&v.id, &mut failure);
            if let ObservationValueV2::Present(roles) = &v.roles {
                for role in roles.as_slice() {
                    member(&role.arn, &mut failure);
                    member(&role.id, &mut failure);
                }
            }
        }
        _ => (),
    }
    failure
}
/// Failed HeadBucket metadata is evidence only when one bounded, valid region is returned.
/// Missing, ambiguous or malformed headers provide no typed region. Never infer ownership.
fn bucket_error_region(
    response: &HttpResponse,
    round: &ObservationRound,
) -> std::result::Result<Option<Region>, LimitKind> {
    if response.status().is_success() {
        return Ok(None);
    }
    let mut first = None;
    let mut duplicate = false;
    for value in response.headers().get_all("x-amz-bucket-region") {
        round
            .remaining()
            .map_err(|_| round.failure().unwrap_or(LimitKind::Clock))?;
        // All occurrences must fit before ambiguity can become an ordinary read failure.
        // Borrowed values are never collected, copied, combined or selected from duplicates.
        if value.len() > 2048 {
            return Err(LimitKind::RecordBytes);
        }
        if first.is_some() {
            duplicate = true;
        } else {
            first = Some(value);
        }
    }
    round
        .remaining()
        .map_err(|_| round.failure().unwrap_or(LimitKind::Clock))?;
    if duplicate {
        return Ok(None);
    }
    Ok(first.and_then(|value| value.parse().ok()))
}
pub(super) fn read_failure<E: ProvideErrorMetadata>(
    error: &SdkError<E, HttpResponse>,
) -> ReadFailureV1 {
    match error {
        // Pinned generated protocol parsers can wrap a malformed successful body
        // in an unhandled service error. HTTP success is not a service rejection.
        SdkError::ServiceError(e) if e.raw().status().is_success() => ReadFailureV1::Malformed,
        SdkError::ServiceError(e) => match e.err().code() {
            Some("ExpiredToken" | "ExpiredTokenException" | "RequestExpired") => {
                ReadFailureV1::SessionExpired
            }
            Some(
                "AccessDenied"
                | "AccessDeniedException"
                | "UnauthorizedOperation"
                | "InvalidClientTokenId"
                | "SignatureDoesNotMatch",
            ) => ReadFailureV1::AccessDenied,
            Some("NoSuchEntity" | "NotFoundException" | "NotFound" | "NoSuchBucket") => {
                ReadFailureV1::NotFound
            }
            _ => match e.raw().status().as_u16() {
                401 | 403 => ReadFailureV1::AccessDenied,
                404 => ReadFailureV1::NotFound,
                _ => ReadFailureV1::Service,
            },
        },
        SdkError::ResponseError(_) | SdkError::ConstructionFailure(_) => ReadFailureV1::Malformed,
        _ => ReadFailureV1::Transport,
    }
}

#[cfg(test)]
#[path = "identity_reads_tests.rs"]
mod tests;
