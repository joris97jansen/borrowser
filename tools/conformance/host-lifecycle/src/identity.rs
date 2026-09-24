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
