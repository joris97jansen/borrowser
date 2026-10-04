//! Bounded allocation members and opaque bytes; no launch authority or SDK types.
use super::{
    coverage::ReadFailureV1,
    ec2_observation_v4::{Ec2LexicalV4, Ec2MemberV4, FactSummaryV4, FactsV4},
    identity_observation_v3::MemberRepresentationFailureV3,
    limits::LimitKind,
    observation::ProviderText,
    source_occurrence_v5::CollectionShapeV5,
};
use crate::{Error, Result, identity::*, require};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

pub const USER_DATA_BYTES: usize = 8192;
pub const USER_DATA_ENCODED_BYTES: usize = 10924;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum MemberV5<T> {
    NotReturned,
    NotExposedBySource,
    Empty,
    Present(T),
    Malformed(ProviderText),
    Unrepresentable(MemberRepresentationFailureV3),
}
impl<T> From<Ec2MemberV4<T>> for MemberV5<T> {
    fn from(value: Ec2MemberV4<T>) -> Self {
        match value {
            Ec2MemberV4::NotReturned => Self::NotReturned,
            Ec2MemberV4::Empty => Self::Empty,
            Ec2MemberV4::Present(v) => Self::Present(v),
            Ec2MemberV4::Malformed(v) => Self::Malformed(v),
            Ec2MemberV4::Unrepresentable(v) => Self::Unrepresentable(v),
        }
    }
}
impl<T: Ec2LexicalV4> FactsV4 for MemberV5<T> {
    fn facts(&self) -> Result<FactSummaryV4> {
        let failure = match self {
            Self::Present(v) => {
                v.validate_literal()?;
                None
            }
            Self::Malformed(raw) => {
                require(
                    !raw.as_str().is_empty() && T::parse_literal(raw.as_str()).is_err(),
                    "malformed allocation member",
                )?;
                Some(ReadFailureV1::Malformed)
            }
            Self::Unrepresentable(MemberRepresentationFailureV3::TextBytes) => {
                Some(ReadFailureV1::Limit(LimitKind::RecordBytes))
            }
            Self::Unrepresentable(MemberRepresentationFailureV3::ContainsNul) => {
                Some(ReadFailureV1::Malformed)
            }
            _ => None,
        };
        Ok(FactSummaryV4 {
            occurrences: 0,
            failure,
        })
    }
}
macro_rules! identities { ($($ty:ty),*) => {$(impl Ec2LexicalV4 for $ty {
    fn parse_literal(value: &str) -> Result<Self> { value.parse() }
})*}; }
identities!(
    AmiId,
    InstanceType,
    VolumeId,
    SnapshotId,
    KmsKeyArn,
    InstanceProfileArn,
    InstanceProfileId,
    ProfileAssociationId,
    EniAttachmentId
);
impl Ec2LexicalV4 for std::net::Ipv4Addr {
    fn parse_literal(value: &str) -> Result<Self> {
        value.parse().map_err(|_| Error("IPv4 address"))
    }
}
impl Ec2LexicalV4 for std::net::Ipv6Addr {
    fn parse_literal(value: &str) -> Result<Self> {
        value.parse().map_err(|_| Error("IPv6 address"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProviderI64V5(i64);
impl From<i64> for ProviderI64V5 {
    fn from(v: i64) -> Self {
        Self(v)
    }
}
impl From<ProviderI64V5> for String {
    fn from(v: ProviderI64V5) -> Self {
        v.0.to_string()
    }
}
impl TryFrom<String> for ProviderI64V5 {
    type Error = Error;
    fn try_from(v: String) -> Result<Self> {
        let n: i64 = v.parse().map_err(|_| Error("provider i64"))?;
        require(n.to_string() == v, "canonical provider i64")?;
        Ok(Self(n))
    }
}
impl FactsV4 for ProviderI64V5 {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4::default())
    }
}

/// Exact SDK f64 bits, including negative zero. Nonfinite values cannot be successful facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProviderF64V5(u64);
impl TryFrom<f64> for ProviderF64V5 {
    type Error = Error;
    fn try_from(value: f64) -> Result<Self> {
        require(value.is_finite(), "nonfinite provider number")?;
        Ok(Self(value.to_bits()))
    }
}
impl From<ProviderF64V5> for String {
    fn from(v: ProviderF64V5) -> Self {
        format!("{:016x}", v.0)
    }
}
impl TryFrom<String> for ProviderF64V5 {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        let bits = u64::from_str_radix(&value, 16).map_err(|_| Error("provider f64 bits"))?;
        require(format!("{bits:016x}") == value, "canonical provider f64")?;
        f64::from_bits(bits).try_into()
    }
}
impl FactsV4 for ProviderF64V5 {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4::default())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderTimeV5 {
    pub seconds: ProviderI64V5,
    pub subsecond_nanos: u32,
}
impl FactsV4 for ProviderTimeV5 {
    fn facts(&self) -> Result<FactSummaryV4> {
        require(
            self.subsecond_nanos < 1_000_000_000,
            "provider timestamp nanos",
        )?;
        Ok(FactSummaryV4::default())
    }
}
impl FactsV4 for CollectionShapeV5 {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4 {
            occurrences: 0,
            failure: match self {
                Self::Present { count } if u64::from(*count) > 128 => {
                    Some(ReadFailureV1::Limit(LimitKind::Records))
                }
                _ => None,
            },
        })
    }
}

/// Canonical Base64 is storage encoding of these bytes, not a second API decode.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UserDataBytesV5(Vec<u8>);
impl std::fmt::Debug for UserDataBytesV5 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserDataBytesV5")
            .field("len", &self.0.len())
            .finish()
    }
}
impl UserDataBytesV5 {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl TryFrom<Vec<u8>> for UserDataBytesV5 {
    type Error = Error;
    fn try_from(value: Vec<u8>) -> Result<Self> {
        require(value.len() <= USER_DATA_BYTES, "user-data decoded bound")?;
        Ok(Self(value))
    }
}
impl From<UserDataBytesV5> for String {
    fn from(v: UserDataBytesV5) -> Self {
        STANDARD.encode(v.0)
    }
}
impl TryFrom<String> for UserDataBytesV5 {
    type Error = Error;
    fn try_from(encoded: String) -> Result<Self> {
        require(
            encoded.len() <= USER_DATA_ENCODED_BYTES,
            "user-data encoded bound",
        )?;
        // Decode into a fixed upper bound, never an input-sized growing output.
        let mut bytes = [0u8; USER_DATA_BYTES + 3];
        let len = STANDARD
            .decode_slice(encoded.as_bytes(), &mut bytes)
            .map_err(|_| Error("user-data Base64"))?;
        require(len <= USER_DATA_BYTES, "user-data decoded bound")?;
        require(
            STANDARD.encode(&bytes[..len]) == encoded,
            "canonical user-data Base64",
        )?;
        Ok(Self(bytes[..len].to_vec()))
    }
}
impl FactsV4 for UserDataBytesV5 {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4::default())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum UserDataValueV5 {
    NotReturned,
    Present(UserDataBytesV5),
    MalformedBase64,
    EncodedLimit,
    DecodedLimit,
}
impl FactsV4 for UserDataValueV5 {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4 {
            occurrences: 0,
            failure: match self {
                Self::MalformedBase64 => Some(ReadFailureV1::Malformed),
                Self::EncodedLimit | Self::DecodedLimit => {
                    Some(ReadFailureV1::Limit(LimitKind::RecordBytes))
                }
                _ => None,
            },
        })
    }
}
