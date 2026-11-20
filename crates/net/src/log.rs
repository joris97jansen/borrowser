use crate::NetworkErrorKind;
use core_types::ResourceKind;
use std::io::Write;

pub(crate) fn log_redirect_decision(
    request_id: u64,
    kind: ResourceKind,
    followed: usize,
    status: crate::HttpStatus,
    decision: &'static str,
) {
    let _ = writeln!(
        std::io::stderr().lock(),
        "{}",
        redirect_message(request_id, kind, followed, status, decision)
    );
}

fn redirect_message(
    request_id: u64,
    kind: ResourceKind,
    followed: usize,
    status: crate::HttpStatus,
    decision: &'static str,
) -> String {
    format!(
        "[net][req={request_id}][{}][stage=redirect][followed={followed}][status={}][decision={decision}]",
        kind.as_str(),
        status.code()
    )
}

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

#[cfg(test)]
mod redirect_tests {
    #[test]
    fn ai7_failure_diagnostic_has_literal_stage_and_category_fields() {
        assert_eq!(
            super::network_error_message(
                7,
                core_types::ResourceKind::Css,
                "body",
                crate::NetworkErrorKind::ResourceLimit(crate::ResourceLimitKind::EncodedBody)
            ),
            "[net][req=7][css][subresource][stage=body][category=ResourceLimit(EncodedBody)]"
        );
        assert_eq!(
            super::network_error_message(
                7,
                core_types::ResourceKind::Html,
                "acquisition",
                crate::NetworkErrorKind::Timeout
            ),
            "[net][req=7][html][top-level][stage=acquisition][category=Timeout]"
        );
    }
    #[test]
    fn ai6_redirect_diagnostic_has_fixed_structural_fields() {
        assert_eq!(
            super::redirect_message(
                7,
                core_types::ResourceKind::Html,
                2,
                crate::HttpStatus::try_from(302).unwrap(),
                "entered"
            ),
            "[net][req=7][html][stage=redirect][followed=2][status=302][decision=entered]"
        );
    }
}
