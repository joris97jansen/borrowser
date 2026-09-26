//! Static normalized categories only; SDK errors never enter journal or diagnostics.
use crate::dispatch::LaunchAttemptOutcome;
use aws_smithy_runtime_api::client::result::SdkError;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryFailure {
    LocalConfiguration,
    ReadOnlyService,
}
impl From<BoundaryFailure> for crate::Error {
    fn from(value: BoundaryFailure) -> Self {
        Self(match value {
            BoundaryFailure::LocalConfiguration => "AWS local configuration/projection rejected",
            BoundaryFailure::ReadOnlyService => "AWS read-only service failed",
        })
    }
}
pub(super) fn after_invocation<R>(
    result: &std::result::Result<
        aws_sdk_ec2::operation::run_instances::RunInstancesOutput,
        SdkError<aws_sdk_ec2::operation::run_instances::RunInstancesError, R>,
    >,
) -> LaunchAttemptOutcome {
    match result {
        Ok(_) => LaunchAttemptOutcome::ResponseUnresolved,
        Err(SdkError::ServiceError(e)) => match e.err().code() {
            Some("IdempotentParameterMismatch") => LaunchAttemptOutcome::ParameterConflict,
            Some(
                "AuthFailure"
                | "UnauthorizedOperation"
                | "InvalidClientTokenId"
                | "ExpiredToken"
                | "ExpiredTokenException"
                | "SignatureDoesNotMatch"
                | "AccessDenied"
                | "AccessDeniedException",
            ) => LaunchAttemptOutcome::AccessBlocked,
            Some("RequestLimitExceeded" | "Throttling" | "ThrottlingException") => {
                LaunchAttemptOutcome::ThrottledHeld
            }
            _ => LaunchAttemptOutcome::ResponseUnresolved,
        },
        // Including dispatch/construction failures: never infer no transmission
        // from a general SDK error after the invocation boundary.
        Err(_) => LaunchAttemptOutcome::TransmissionUncertain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_ec2::operation::run_instances::{RunInstancesError, RunInstancesOutput};
    #[test]
    fn synthetic_sdk_results_never_invent_nontransmission() {
        type Outcome = std::result::Result<RunInstancesOutput, SdkError<RunInstancesError, ()>>;
        for (code, expected) in [
            (
                "IdempotentParameterMismatch",
                LaunchAttemptOutcome::ParameterConflict,
            ),
            ("UnauthorizedOperation", LaunchAttemptOutcome::AccessBlocked),
            ("ExpiredToken", LaunchAttemptOutcome::AccessBlocked),
            ("RequestLimitExceeded", LaunchAttemptOutcome::ThrottledHeld),
            (
                "UnknownProviderCode",
                LaunchAttemptOutcome::ResponseUnresolved,
            ),
        ] {
            let error = RunInstancesError::generic(
                aws_smithy_types::error::ErrorMetadata::builder()
                    .code(code)
                    .message("unrestricted provider data must not escape")
                    .build(),
            );
            let result: Outcome = Err(SdkError::service_error(error, ()));
            assert_eq!(after_invocation(&result), expected);
            assert!(!format!("{:?}", after_invocation(&result)).contains("provider data"));
        }
        let result: Outcome = Err(SdkError::timeout_error("timeout"));
        assert_eq!(
            after_invocation(&result),
            LaunchAttemptOutcome::TransmissionUncertain
        );
        let result: Outcome = Err(SdkError::construction_failure("construction"));
        assert_eq!(
            after_invocation(&result),
            LaunchAttemptOutcome::TransmissionUncertain
        );
        let result: Outcome = Ok(RunInstancesOutput::builder().build());
        assert_eq!(
            after_invocation(&result),
            LaunchAttemptOutcome::ResponseUnresolved
        );
    }
}
