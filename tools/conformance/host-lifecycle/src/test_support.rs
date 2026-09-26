//! Synthetic storage-test artifacts. Never compiled into production.
use crate::{canonical, deployment::AuthorityRootV2, model::EnvelopeV2};
pub(crate) fn marker() -> AuthorityRootV2 {
    canonical::decode(include_bytes!("../tests/fixtures/authority-v2.json")).unwrap()
}
pub(crate) fn genesis() -> EnvelopeV2 {
    canonical::decode(include_bytes!("../tests/fixtures/genesis-v2.json")).unwrap()
}

pub(crate) fn launch_documents() -> crate::dispatch::PreparedLaunchV2 {
    crate::dispatch::PreparedLaunchV2 {
        deployment: canonical::decode(include_bytes!(
            "../tests/fixtures/reviewed-deployment-v2.json"
        ))
        .unwrap(),
        approval: canonical::decode(include_bytes!("../tests/fixtures/launch-approval-v2.json"))
            .unwrap(),
        trust: canonical::decode(include_bytes!("../tests/fixtures/identity-trust-v2.json"))
            .unwrap(),
        specification: canonical::decode(include_bytes!("../tests/fixtures/launch-spec-v2.json"))
            .unwrap(),
        request: canonical::decode(include_bytes!(
            "../tests/fixtures/run-instances-request-v2.json"
        ))
        .unwrap(),
        authorization: canonical::decode(include_bytes!(
            "../tests/fixtures/dispatch-v2/authorization.json"
        ))
        .unwrap(),
    }
}
