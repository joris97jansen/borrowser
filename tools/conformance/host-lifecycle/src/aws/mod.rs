//! Explicit, internal, non-mutating AWS boundary for #1396.
mod credentials;
mod errors;
mod projection;
use crate::{
    Error, Result,
    deployment::DeploymentV2,
    identity::{AwsAccountId, EvidenceBucketName, Region},
    require,
};
use aws_smithy_runtime_api::client::http::SharedHttpClient;
use aws_smithy_types::{retry::RetryConfig, timeout::TimeoutConfig};
use credentials::SessionSecret;
use std::{
    path::Path,
    time::{Duration, SystemTime},
};

const BEHAVIOR: &str = "2026-01-12";
fn configuration(
    secret: &SessionSecret,
    region: &Region,
    http: SharedHttpClient,
) -> Result<aws_types::SdkConfig> {
    Ok(aws_types::SdkConfig::builder()
        .behavior_version(
            aws_smithy_runtime_api::client::behavior_version::BehaviorVersion::v2026_01_12(),
        )
        .region(aws_types::region::Region::new(region.to_string()))
        .credentials_provider(
            aws_credential_types::provider::SharedCredentialsProvider::new(
                secret.sdk(SystemTime::now())?,
            ),
        )
        .retry_config(RetryConfig::standard().with_max_attempts(1))
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(5))
                .operation_attempt_timeout(Duration::from_secs(30))
                .operation_timeout(Duration::from_secs(30))
                .build(),
        )
        .http_client(http)
        .sleep_impl(aws_smithy_async::rt::sleep::TokioSleep::new())
        .time_source(aws_smithy_async::time::SystemTimeSource::new())
        .build())
}
// The explicit HTTP builder uses disabled proxies; unlike the SDK default client
// factory, it never calls ProxyConfig::from_env. TLS verification stays enabled.
fn production_http() -> SharedHttpClient {
    aws_smithy_http_client::Builder::new()
        .tls_provider(aws_smithy_http_client::tls::Provider::Rustls(
            aws_smithy_http_client::tls::rustls_provider::CryptoMode::AwsLc,
        ))
        .build_https()
}
#[derive(Debug, PartialEq, Eq)]
struct CallerIdentity {
    account: AwsAccountId,
    arn: String,
    user_id: String,
}
impl CallerIdentity {
    fn normalize(
        account: Option<&str>,
        arn: Option<&str>,
        user: Option<&str>,
        expected: &AwsAccountId,
    ) -> Result<Self> {
        let account: AwsAccountId = account.ok_or(Error("caller account missing"))?.parse()?;
        require(&account == expected, "caller account mismatch")?;
        let arn = bounded(arn, 2048)?;
        let user_id = bounded(user, 256)?;
        let parts: Vec<_> = arn.splitn(6, ':').collect();
        require(
            parts.len() == 6
                && parts[0] == "arn"
                && !parts[1].is_empty()
                && matches!(parts[2], "iam" | "sts")
                && parts[3].is_empty()
                && parts[4] == account.as_str()
                && !parts[5].is_empty()
                && parts[5] != "root",
            "caller principal rejected",
        )?;
        Ok(Self {
            account,
            arn,
            user_id,
        })
    }
}
fn bounded(value: Option<&str>, limit: usize) -> Result<String> {
    let value = value.ok_or(Error("provider identity missing"))?;
    require(
        !value.is_empty() && value.len() <= limit && value.bytes().all(|b| b.is_ascii_graphic()),
        "provider identity invalid",
    )?;
    Ok(value.to_owned())
}
#[derive(Debug, PartialEq, Eq)]
struct BucketIdentity {
    bucket: EvidenceBucketName,
    account: AwsAccountId,
    region: Region,
}
// No Debug/Serialize or client accessors. SDK clients remain encapsulated.
struct AwsSession {
    caller: CallerIdentity,
    region: Region,
    expires: SystemTime,
    ec2: aws_sdk_ec2::Client,
    s3: aws_sdk_s3::Client,
}
impl AwsSession {
    async fn open(deployment: &DeploymentV2, path: &Path) -> Result<Self> {
        deployment.support()?;
        let secret = SessionSecret::load(path, SystemTime::now())?;
        Self::admit(deployment, secret, production_http()).await
    }
    async fn admit(
        deployment: &DeploymentV2,
        secret: SessionSecret,
        http: SharedHttpClient,
    ) -> Result<Self> {
        deployment.support()?;
        let conf = configuration(&secret, &deployment.identity.region, http)?;
        let sts = aws_sdk_sts::Client::from_conf(aws_sdk_sts::config::Builder::from(&conf).build());
        let result = sts
            .get_caller_identity()
            .send()
            .await
            .map_err(|_| errors::BoundaryFailure::ReadOnlyService)?;
        let caller = CallerIdentity::normalize(
            result.account(),
            result.arn(),
            result.user_id(),
            &deployment.identity.account_id,
        )?;
        require(SystemTime::now() < secret.expires(), "session expired")?;
        Ok(Self {
            caller,
            region: deployment.identity.region.clone(),
            expires: secret.expires(),
            ec2: aws_sdk_ec2::Client::from_conf(aws_sdk_ec2::config::Builder::from(&conf).build()),
            s3: aws_sdk_s3::Client::from_conf(
                aws_sdk_s3::config::Builder::from(&conf)
                    .disable_s3_express_session_auth(true)
                    .build(),
            ),
        })
    }
    async fn head_evidence_bucket(&self, deployment: &DeploymentV2) -> Result<BucketIdentity> {
        let support = deployment.support()?;
        require(
            self.caller.account == deployment.identity.account_id
                && self.region == deployment.identity.region,
            "session deployment mismatch",
        )?;
        require(SystemTime::now() < self.expires, "session expired")?;
        let result = self
            .s3
            .head_bucket()
            .bucket(support.evidence_bucket.as_str())
            .expected_bucket_owner(self.caller.account.as_str())
            .send()
            .await
            .map_err(|_| errors::BoundaryFailure::ReadOnlyService)?;
        let region: Region = bounded(result.bucket_region(), 32)?.parse()?;
        require(
            region == self.region
                && region == support.evidence_bucket_region
                && region == support.s3_gateway_endpoint_region,
            "bucket region mismatch",
        )?;
        Ok(BucketIdentity {
            bucket: support.evidence_bucket.clone(),
            account: self.caller.account.clone(),
            region,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_smithy_http_client::test_util::{ReplayEvent, StaticReplayClient};
    use aws_smithy_types::body::SdkBody;
    use std::os::unix::fs::PermissionsExt;
    fn secret() -> SessionSecret {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("session");
        std::fs::write(&path,br#"{"format":"borrowser-aws-operator-session","schema_version":1,"access_key_id":"SYNTHETICACCESS","secret_access_key":"synthetic-secret","session_token":"synthetic-token","expiration_unix_seconds":4102444800}"#).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        SessionSecret::load(&path, SystemTime::now()).unwrap()
    }
    fn response(status: u16, body: &str, region: Option<&str>) -> http::Response<SdkBody> {
        let mut b = http::Response::builder().status(status);
        if let Some(r) = region {
            b = b.header("x-amz-bucket-region", r);
        }
        b.body(SdkBody::from(body)).unwrap()
    }
    fn replay(responses: Vec<http::Response<SdkBody>>) -> StaticReplayClient {
        StaticReplayClient::new(
            responses
                .into_iter()
                .map(|r| {
                    ReplayEvent::new(
                        http::Request::builder()
                            .uri("https://synthetic.invalid")
                            .body(SdkBody::empty())
                            .unwrap(),
                        r,
                    )
                })
                .collect(),
        )
    }
    fn caller_xml(account: &str, arn: &str) -> String {
        format!(
            "<GetCallerIdentityResponse xmlns=\"https://sts.amazonaws.com/doc/2011-06-15/\"><GetCallerIdentityResult><Arn>{arn}</Arn><UserId>synthetic-user:session</UserId><Account>{account}</Account></GetCallerIdentityResult><ResponseMetadata><RequestId>synthetic</RequestId></ResponseMetadata></GetCallerIdentityResponse>"
        )
    }
    #[test]
    fn pinned_source_inventory_and_private_boundary() {
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../tests/fixtures/sdk-members-v1.json")).unwrap();
        assert_eq!(rows.len(), 157);
        let identities: std::collections::BTreeSet<_> = rows
            .iter()
            .map(|r| (r["type"].as_str().unwrap(), r["member"].as_str().unwrap()))
            .collect();
        assert_eq!(identities.len(), 157);
        assert_eq!(
            identities
                .iter()
                .map(|p| p.0)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            35
        );
        assert!(
            rows.iter()
                .any(|r| r["member"] == "connection_tracking_specification"
                    && r["disposition"] == "Unsupported")
        );
        assert!(!rows.iter().any(|r| r["member"] == "reboot_migration"));
        let lock = include_str!("../../Cargo.lock");
        for (name, version) in [
            ("aws-sdk-ec2", "1.237.0"),
            ("aws-sdk-sts", "1.107.0"),
            ("aws-sdk-s3", "1.137.0"),
        ] {
            assert!(lock.contains(&format!("name = \"{name}\"\nversion = \"{version}\"")));
        }
        for name in ["aws-config", "aws-sdk-iam", "aws-sdk-kms"] {
            assert!(!lock.contains(&format!("name = \"{name}\"")));
        }
    }
    #[tokio::test]
    async fn explicit_configs_and_synthetic_sts_s3_admission() {
        let d = crate::test_support::launch_documents().deployment;
        let account = d.identity.account_id.as_str();
        let arn = format!("arn:aws:sts::{account}:assumed-role/synthetic/session");
        let connector = replay(vec![
            response(200, &caller_xml(account, &arn), None),
            response(200, "", Some(d.identity.region.as_str())),
        ]);
        let conf = configuration(
            &secret(),
            &d.identity.region,
            SharedHttpClient::new(connector.clone()),
        )
        .unwrap();
        macro_rules! check_config {
            ($service:ident) => {{
                let c = $service::config::Builder::from(&conf).build();
                assert_eq!(c.region().unwrap().as_ref(), d.identity.region.as_str());
                assert_eq!(c.retry_config().unwrap().max_attempts(), 1);
                assert_eq!(
                    c.timeout_config().unwrap().connect_timeout(),
                    Some(Duration::from_secs(5))
                );
                assert_eq!(
                    c.timeout_config().unwrap().operation_attempt_timeout(),
                    Some(Duration::from_secs(30))
                );
                assert_eq!(
                    c.timeout_config().unwrap().operation_timeout(),
                    Some(Duration::from_secs(30))
                );
            }};
        }
        check_config!(aws_sdk_sts);
        check_config!(aws_sdk_s3);
        check_config!(aws_sdk_ec2);
        assert_eq!(BEHAVIOR, "2026-01-12");
        assert!(
            conf.behavior_version().unwrap()
                == aws_smithy_runtime_api::client::behavior_version::BehaviorVersion::v2026_01_12()
        );
        let session = AwsSession::admit(&d, secret(), SharedHttpClient::new(connector.clone()))
            .await
            .unwrap();
        assert_eq!(session.caller.arn, arn);
        assert_eq!(session.caller.user_id, "synthetic-user:session");
        let bucket = session.head_evidence_bucket(&d).await.unwrap();
        assert_eq!(bucket.region, d.identity.region);
        let requests: Vec<_> = connector.actual_requests().collect();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[0]
                .uri()
                .to_string()
                .contains(d.identity.region.as_str())
        );
        assert!(
            std::str::from_utf8(requests[0].body().bytes().unwrap())
                .unwrap()
                .contains("Action=GetCallerIdentity")
        );
        assert_eq!(requests[1].method(), "HEAD");
        assert_eq!(
            requests[1].headers().get("x-amz-expected-bucket-owner"),
            Some(account)
        );
        assert!(
            requests[1]
                .uri()
                .to_string()
                .contains(d.support().unwrap().evidence_bucket.as_str())
        );
        assert!(
            !requests
                .iter()
                .any(|r| r.uri().to_string().contains("synthetic.invalid"))
        );
    }
    #[tokio::test]
    async fn synthetic_failures_do_not_retry_or_admit() {
        let d = crate::test_support::launch_documents().deployment;
        let account = d.identity.account_id.as_str();
        for (status, body) in [
            (
                403,
                "<ErrorResponse><Error><Code>AccessDenied</Code></Error></ErrorResponse>"
                    .to_owned(),
            ),
            (
                200,
                caller_xml("999999999999", "arn:aws:iam::999999999999:user/test"),
            ),
            (
                200,
                caller_xml(account, &format!("arn:aws:iam::{account}:root")),
            ),
            (200, caller_xml(account, "malformed")),
        ] {
            let c = replay(vec![response(status, &body, None)]);
            assert!(
                AwsSession::admit(&d, secret(), SharedHttpClient::new(c.clone()))
                    .await
                    .is_err()
            );
            assert_eq!(c.actual_requests().count(), 1);
        }
        for (status, region) in [
            (200, None),
            (200, Some("wrong-region-1")),
            (403, Some("wrong-region-1")),
            (503, None),
        ] {
            let c = replay(vec![
                response(
                    200,
                    &caller_xml(account, &format!("arn:aws:iam::{account}:user/synthetic")),
                    None,
                ),
                response(status, "", region),
            ]);
            let s = AwsSession::admit(&d, secret(), SharedHttpClient::new(c.clone()))
                .await
                .unwrap();
            assert!(s.head_evidence_bucket(&d).await.is_err());
            assert_eq!(c.actual_requests().count(), 2);
        }
        // Even a directory-bucket-shaped reviewed name cannot induce CreateSession.
        let mut directory_deployment = d.clone();
        directory_deployment
            .reviewed_support
            .as_mut()
            .unwrap()
            .evidence_bucket = "synthetic--euc1-az1--x-s3".parse().unwrap();
        let connector = replay(vec![
            response(
                200,
                &caller_xml(account, &format!("arn:aws:iam::{account}:user/synthetic")),
                None,
            ),
            response(200, "", Some(d.identity.region.as_str())),
        ]);
        let session = AwsSession::admit(
            &directory_deployment,
            secret(),
            SharedHttpClient::new(connector.clone()),
        )
        .await
        .unwrap();
        session
            .head_evidence_bucket(&directory_deployment)
            .await
            .unwrap();
        assert_eq!(connector.actual_requests().count(), 2);
        assert_eq!(connector.actual_requests().nth(1).unwrap().method(), "HEAD");
        assert!(
            CallerIdentity::normalize(
                Some(account),
                Some(&"x".repeat(2049)),
                Some("u"),
                &d.identity.account_id
            )
            .is_err()
        );
        assert!(
            CallerIdentity::normalize(
                Some(account),
                Some(&format!("arn:aws:iam::{account}:user/x")),
                Some("bad\nuser"),
                &d.identity.account_id
            )
            .is_err()
        );
    }
    #[test]
    fn ambient_settings_are_ignored_in_an_isolated_child() {
        const CHILD: &str = "BORROWSER_SDK_AMBIENT_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let d = crate::test_support::launch_documents().deployment;
                let account = d.identity.account_id.as_str();
                let c = replay(vec![response(
                    200,
                    &caller_xml(account, &format!("arn:aws:iam::{account}:user/synthetic")),
                    None,
                )]);
                AwsSession::admit(&d, secret(), SharedHttpClient::new(c.clone()))
                    .await
                    .unwrap();
                let r = c.actual_requests().next().unwrap();
                assert!(!r.uri().to_string().contains("ambient.invalid"));
                assert!(
                    r.headers()
                        .get("authorization")
                        .unwrap()
                        .contains("SYNTHETICACCESS")
                );
                assert_eq!(
                    r.headers().get("x-amz-security-token"),
                    Some("synthetic-token")
                );
            });
            return;
        }
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "aws::tests::ambient_settings_are_ignored_in_an_isolated_child",
            ])
            .env(CHILD, "1");
        for name in [
            "AWS_PROFILE",
            "AWS_DEFAULT_PROFILE",
            "AWS_REGION",
            "AWS_DEFAULT_REGION",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_ENDPOINT_URL",
            "AWS_ENDPOINT_URL_STS",
            "AWS_ENDPOINT_URL_EC2",
            "AWS_ENDPOINT_URL_S3",
            "AWS_CONFIG_FILE",
            "AWS_SHARED_CREDENTIALS_FILE",
            "AWS_MAX_ATTEMPTS",
            "AWS_RETRY_MODE",
        ] {
            command.env(name, "https://ambient.invalid");
        }
        assert!(command.status().unwrap().success());
    }
}
