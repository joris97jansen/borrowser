use crate::{Sha256Digest, UpstreamPath};
use std::fmt;
use std::num::NonZeroU64;

macro_rules! non_empty_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, NonEmptyIdentityError> {
                if value.is_empty() || value.trim() != value {
                    return Err(NonEmptyIdentityError);
                }
                Ok(Self(value.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

non_empty_identity!(UpstreamProjectId);
non_empty_identity!(LicenseIdentifier);
non_empty_identity!(LicenseNotice);
non_empty_identity!(Attribution);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NonEmptyIdentityError;

impl fmt::Display for NonEmptyIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("identity value must be non-empty without surrounding whitespace")
    }
}

impl std::error::Error for NonEmptyIdentityError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImmutableRevision(String);

impl ImmutableRevision {
    pub fn parse(value: &str) -> Result<Self, RevisionParseError> {
        if value.is_empty()
            || value.len() > 256
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(RevisionParseError::InvalidStableIdentifier);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn parse_git_commit(value: &str) -> Result<Self, RevisionParseError> {
        if value.len() != 40
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RevisionParseError::InvalidGitCommit);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevisionParseError {
    InvalidStableIdentifier,
    InvalidGitCommit,
}

impl fmt::Display for RevisionParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStableIdentifier => {
                formatter.write_str("immutable revision must be a stable normalized identifier")
            }
            Self::InvalidGitCommit => formatter.write_str(
                "immutable Git revision must be a 40-character lowercase hexadecimal commit",
            ),
        }
    }
}

impl std::error::Error for RevisionParseError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalFileIdentity {
    project: UpstreamProjectId,
    revision: ImmutableRevision,
    path: UpstreamPath,
    sha256: Sha256Digest,
}

impl ExternalFileIdentity {
    pub fn new(
        project: UpstreamProjectId,
        revision: ImmutableRevision,
        path: UpstreamPath,
        sha256: Sha256Digest,
    ) -> Self {
        Self {
            project,
            revision,
            path,
            sha256,
        }
    }

    pub fn project(&self) -> &UpstreamProjectId {
        &self.project
    }
    pub fn revision(&self) -> &ImmutableRevision {
        &self.revision
    }
    pub fn path(&self) -> &UpstreamPath {
        &self.path
    }
    pub fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalRecordSelector {
    WholeFile,
    IndexedRecord {
        ordinal: NonZeroU64,
        sha256: Sha256Digest,
    },
}
