//! Distinct validated identities. Wire representations stay JSON strings/u64.
use crate::{Result, canonical, require};
use serde::{Deserialize, Serialize};
fn scoped(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b)),
        "scope identity syntax",
    )
}
fn transaction(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Robot transaction identity syntax",
    )
}
fn product(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "Robot product identity syntax",
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
identity!(AccountScopeId, scoped);
identity!(OperationId, scoped);
identity!(RobotTransactionId, transaction);
identity!(ProductId, product);
identity!(RequestFingerprint, canonical::digest);
identity!(EventDigest, canonical::digest);
identity!(EvidenceDigest, canonical::digest);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct ServerNumber(u64);
impl TryFrom<u64> for ServerNumber {
    type Error = crate::Error;
    fn try_from(n: u64) -> Result<Self> {
        require(n > 0, "zero server number")?;
        Ok(Self(n))
    }
}
impl From<ServerNumber> for u64 {
    fn from(n: ServerNumber) -> Self {
        n.0
    }
}
impl ServerNumber {
    pub fn get(self) -> u64 {
        self.0
    }
}
impl std::fmt::Display for ServerNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
