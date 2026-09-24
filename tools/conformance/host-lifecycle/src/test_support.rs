//! Synthetic storage-test artifacts. Never compiled into production.
use crate::{canonical, deployment::AuthorityRootV2, model::EnvelopeV2};
pub(crate) fn marker() -> AuthorityRootV2 {
    canonical::decode(include_bytes!("../tests/fixtures/authority-v2.json")).unwrap()
}
pub(crate) fn genesis() -> EnvelopeV2 {
    canonical::decode(include_bytes!("../tests/fixtures/genesis-v2.json")).unwrap()
}
