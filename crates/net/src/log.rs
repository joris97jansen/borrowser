use crate::NetworkErrorKind;
use core_types::ResourceKind;
use std::io::Write;

pub(crate) fn log_network_error(
    request_id: u64,
    kind: ResourceKind,
    stage: &'static str,
    category: NetworkErrorKind,
) {
    // Diagnostics are optional: an unavailable sink must not abort the worker
    // before its terminal network event is delivered.
    let _ = writeln!(
        std::io::stderr().lock(),
        "{}",
        network_error_message(request_id, kind, stage, category)
    );
}

fn network_error_message(
    request_id: u64,
    kind: ResourceKind,
    stage: &'static str,
    category: NetworkErrorKind,
) -> String {
    format!(
        "[net][req={request_id}][{}][{}][stage={stage}][category={category:?}]",
        kind.as_str(),
        kind.role_str(),
    )
}
