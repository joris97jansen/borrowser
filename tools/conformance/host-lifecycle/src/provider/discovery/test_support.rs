use crate::{canonical, dispatch::PreparedLaunchV2, provider::context_v3::*};
pub(crate) const MANIFEST: &[u8] =
    include_bytes!("../../../tests/fixtures/provider-foundation-v1/manifest.json");
pub(crate) fn retained() -> (PreparedLaunchV2, ReconciliationContextV3) {
    let mut p = crate::test_support::launch_documents();
    let digest = canonical::sha256(MANIFEST).parse().unwrap();
    p.deployment
        .reviewed_support
        .as_mut()
        .unwrap()
        .infrastructure_sha256 = digest;
    p.approval.deployment_sha256 = p.deployment.digest().unwrap();
    p.approval.infrastructure_sha256 = p
        .deployment
        .support()
        .unwrap()
        .infrastructure_sha256
        .clone();
    p.specification = crate::launch::LaunchSpecV2::new(
        &p.deployment,
        &p.approval,
        &p.trust,
        p.specification.operation_id.clone(),
    )
    .unwrap();
    let token =
        crate::launch::ClientToken::derive(&p.specification, &p.deployment, &p.approval, &p.trust)
            .unwrap();
    p.request = crate::launch::RunInstancesRequestV2::new(
        &p.deployment,
        &p.approval,
        &p.trust,
        p.specification.clone(),
        token,
    )
    .unwrap();
    p.authorization.binding = p.binding().unwrap();
    let mut c = ReconciliationContextV3::parse(include_bytes!(
        "../../../tests/fixtures/provider-foundation-v3/context.json"
    ))
    .unwrap()
    .fields()
    .clone();
    c.binding = p.binding().unwrap();
    c.artifacts = p.references().unwrap();
    c.root = p.deployment.marker().unwrap();
    c.root_sha256 = c.root.digest().unwrap();
    c.manifest = p.approval.infrastructure_sha256.clone();
    c.prior_provider = None;
    c.dispatch = None;
    c.attempts.clear();
    (p, ReconciliationContextV3::from_fields(c).unwrap())
}
