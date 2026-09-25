//! Validated local authority identities. Parsed values grant no mutation authority.
use crate::{Result, canonical, require};
use serde::{Deserialize, Serialize};
fn scoped(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b)),
        "authority identity syntax",
    )
}
fn account(s: &str) -> Result<()> {
    require(
        s.len() == 12 && s.bytes().all(|b| b.is_ascii_digit()),
        "AWS account identity syntax",
    )
}
fn region(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 32
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !s.starts_with('-')
            && !s.ends_with('-')
            && !s.contains("--"),
        "AWS region syntax",
    )
}
macro_rules! identity {
    ($name:ident, $check:path) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);
        impl TryFrom<String> for $name {
            type Error = crate::Error;
            fn try_from(s: String) -> Result<Self> {
                $check(&s)?;
                Ok(Self(s))
            }
        }
        impl std::str::FromStr for $name {
            type Err = crate::Error;
            fn from_str(s: &str) -> Result<Self> {
                Self::try_from(s.to_owned())
            }
        }
        impl From<$name> for String {
            fn from(s: $name) -> Self {
                s.0
            }
        }
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
identity!(AuthorityId, scoped);
identity!(AwsAccountId, account);
identity!(Region, region);
identity!(EventDigest, canonical::digest);

identity!(AuthorityRootDigest, canonical::digest);

/// Root and event hashes share a wire encoding, but never an implicit Rust conversion.
/// ```compile_fail
/// use borrowser_host_lifecycle::identity::{AuthorityRootDigest, EventDigest};
/// let root: AuthorityRootDigest = "a".repeat(64).parse().unwrap();
/// let event: EventDigest = root.into();
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::identity::{AuthorityRootDigest, EventDigest};
/// let event: EventDigest = "a".repeat(64).parse().unwrap();
/// let root: AuthorityRootDigest = event.into();
/// ```
const _: () = ();

// AWS identifiers are lexical constraints, never catalogue/admission checks.
fn resource(s: &str, prefix: &str) -> Result<()> {
    let tail = s
        .strip_prefix(prefix)
        .ok_or(crate::Error("resource prefix"))?;
    require(
        (1..=64).contains(&tail.len())
            && tail
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "resource identity syntax",
    )
}
fn ami(s: &str) -> Result<()> {
    resource(s, "ami-")
}
fn vpc(s: &str) -> Result<()> {
    resource(s, "vpc-")
}
fn subnet(s: &str) -> Result<()> {
    resource(s, "subnet-")
}
fn endpoint(s: &str) -> Result<()> {
    resource(s, "vpce-")
}
fn route_table(s: &str) -> Result<()> {
    resource(s, "rtb-")
}
fn security_group(s: &str) -> Result<()> {
    resource(s, "sg-")
}
fn instance_type(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b))
            && s.split(['.', '-']).all(|v| !v.is_empty()),
        "instance type syntax",
    )
}
fn bucket(s: &str) -> Result<()> {
    // Deliberately bounded DNS-label subset; actual bucket ownership is not lexical.
    require(
        (3..=63).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && s.bytes().any(|b| b.is_ascii_lowercase())
            && !s.starts_with('-')
            && !s.ends_with('-'),
        "bucket syntax",
    )
}
// Unique-ID prefixes/length history do not define role/profile authority.
fn iam_unique_id(s: &str) -> Result<()> {
    require(
        (16..=128).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "IAM unique identity syntax",
    )
}
fn arn(s: &str, service: &str, prefix: &str, regional: bool) -> Result<()> {
    require(s.len() <= 512, "ARN bound")?;
    let p: Vec<_> = s.split(':').collect();
    require(p.len() == 6, "ARN structure")?;
    require(p[0] == "arn" && p[2] == service, "ARN service")?;
    region(p[1])?;
    if regional {
        region(p[3])?;
    } else {
        require(p[3].is_empty(), "IAM ARN region")?;
    }
    account(p[4])?;
    let name = p[5]
        .strip_prefix(prefix)
        .ok_or(crate::Error("ARN resource"))?;
    require(
        !name.is_empty()
            && name.split('/').all(|v| !v.is_empty())
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/+=,.@_-".contains(&b)),
        "ARN resource syntax",
    )
}
fn profile(s: &str) -> Result<()> {
    arn(s, "iam", "instance-profile/", false)
}
fn role(s: &str) -> Result<()> {
    arn(s, "iam", "role/", false)
}
fn kms(s: &str) -> Result<()> {
    arn(s, "kms", "key/", true)?;
    let key = s
        .split(':')
        .nth(5)
        .and_then(|r| r.strip_prefix("key/"))
        .ok_or(crate::Error("KMS key"))?;
    // Exact key identity only, never a mutable alias. Includes multi-region key IDs.
    let valid = if let Some(hex) = key.strip_prefix("mrk-") {
        hex.len() == 32
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    } else {
        key.len() == 36
            && key.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }
            })
    };
    require(valid, "KMS key identity")
}
fn review_text(s: &str) -> Result<()> {
    canonical::text(s, 256)
}
identity!(OperationId, scoped);
identity!(AmiId, ami);
identity!(VpcId, vpc);
identity!(SubnetId, subnet);
identity!(SecurityGroupId, security_group);
identity!(VpcEndpointId, endpoint);
identity!(RouteTableId, route_table);
identity!(AvailabilityZone, region);
identity!(AvailabilityZoneId, region);
identity!(InstanceType, instance_type);
identity!(InstanceProfileArn, profile);
identity!(InstanceProfileId, iam_unique_id);
identity!(IamRoleArn, role);
identity!(IamRoleId, iam_unique_id);
identity!(KmsKeyArn, kms);
identity!(EvidenceBucketName, bucket);
identity!(ReviewText, review_text);
identity!(DeploymentDigest, canonical::digest);
identity!(InfrastructureDigest, canonical::digest);
identity!(LaunchSpecDigest, canonical::digest);
identity!(LaunchRequestDigest, canonical::digest);
identity!(LaunchApprovalDigest, canonical::digest);
identity!(IdentityTrustDigest, canonical::digest);
identity!(CertificateDigest, canonical::digest);
identity!(PricingEvidenceDigest, canonical::digest);
identity!(AmiProvenanceDigest, canonical::digest);

/// Binding checks do not query IAM/KMS or establish deployed identity.
pub(crate) fn arn_binding(s: &str, account: &AwsAccountId, region: Option<&Region>) -> Result<()> {
    let p: Vec<_> = s.split(':').collect();
    require(
        p.len() == 6 && p[4] == account.as_str() && region.is_none_or(|r| p[3] == r.as_str()),
        "ARN account/region binding",
    )
}
