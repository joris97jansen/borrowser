use super::aggregate::Failure;
use std::io::Write;

pub(super) struct PreparedArtifact {
    pub bytes: Vec<u8>,
    pub policy_failed: bool,
}

#[allow(
    clippy::result_large_err,
    reason = "Preserve closed typed registry diagnostics without allocating while reporting failure"
)]
pub(super) fn publish(artifact: PreparedArtifact, output: &mut impl Write) -> Result<i32, Failure> {
    output.write_all(&artifact.bytes).map_err(Failure::Output)?;
    output.flush().map_err(Failure::Output)?;
    Ok(i32::from(artifact.policy_failed))
}
