//! Source-neutral identity and integrity primitives for pinned external tests.
//!
//! This crate deliberately knows nothing about AG policy, WPT source forms,
//! Borrowser fixture identifiers, or production engine semantics.

mod confined_file;
mod digest;
mod identity;
mod path;
mod provenance_v1;

pub use confined_file::{
    ConfinedFileError, read_confined_regular_file, validate_confined_output_file,
    validate_confined_regular_file,
};
pub use digest::{DigestParseError, Sha256Digest, sha256};
pub use identity::{
    Attribution, ExternalFileIdentity, ExternalRecordSelector, ImmutableRevision,
    LicenseIdentifier, LicenseNotice, NonEmptyIdentityError, RevisionParseError, UpstreamProjectId,
};
pub use path::{UpstreamPath, UpstreamPathParseError};
pub use provenance_v1::{
    EXTERNAL_PROVENANCE_FORMAT_V1, ExternalProvenanceV1, ExternalProvenanceV1Error,
    parse_external_provenance_v1, serialize_external_provenance_v1,
};
