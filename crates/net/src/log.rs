use core_types::ResourceKind;

pub(crate) fn log_network_error(
    request_id: u64,
    kind: ResourceKind,
    url: &str,
    stage: &str,
    detail: &str,
) {
    eprintln!(
        "[net][req={request_id}][{}][{}][{stage}] {url}: {detail}",
        kind.as_str(),
        kind.role_str(),
    );
}
