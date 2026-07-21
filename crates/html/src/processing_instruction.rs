/// Validation failures for Borrowser's parser-produced HTML processing
/// instruction payload domain.
///
/// This is intentionally narrower than the future DOM constructor domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserCreatedProcessingInstructionError {
    EmptyTarget,
    InvalidTargetStart,
    InvalidTargetContinuation,
    DisallowedTarget,
    DataContainsGreaterThan,
}

pub fn validate_parser_created_processing_instruction(
    target: &str,
    data: &str,
) -> Result<(), ParserCreatedProcessingInstructionError> {
    let mut bytes = target.bytes();
    let Some(first) = bytes.next() else {
        return Err(ParserCreatedProcessingInstructionError::EmptyTarget);
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return Err(ParserCreatedProcessingInstructionError::InvalidTargetStart);
    }
    if !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
        return Err(ParserCreatedProcessingInstructionError::InvalidTargetContinuation);
    }
    if target.eq_ignore_ascii_case("xml") || target.eq_ignore_ascii_case("xml-stylesheet") {
        return Err(ParserCreatedProcessingInstructionError::DisallowedTarget);
    }
    if data.contains('>') {
        return Err(ParserCreatedProcessingInstructionError::DataContainsGreaterThan);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Id, ProcessingInstructionNode};

    #[test]
    fn parser_created_domain_preserves_case_but_rejects_forbidden_targets_case_insensitively() {
        assert_eq!(
            validate_parser_created_processing_instruction("Pi", "data"),
            Ok(())
        );
        assert_eq!(
            validate_parser_created_processing_instruction("Xml-StyleSheet", ""),
            Err(ParserCreatedProcessingInstructionError::DisallowedTarget)
        );
    }

    #[test]
    fn materialized_node_factory_enforces_validity_without_debug_only_checks() {
        let valid = ProcessingInstructionNode::try_from_parser_created_parts(
            Id::INVALID,
            "Pi".to_string(),
            "data".to_string(),
        )
        .expect("valid parser-created PI");
        assert_eq!(valid.target(), "Pi");
        assert_eq!(valid.data(), "data");

        assert_eq!(
            ProcessingInstructionNode::try_from_parser_created_parts(
                Id::INVALID,
                "xml".to_string(),
                "data".to_string(),
            )
            .unwrap_err(),
            ParserCreatedProcessingInstructionError::DisallowedTarget
        );
        assert_eq!(
            ProcessingInstructionNode::try_from_parser_created_parts(
                Id::INVALID,
                "pi".to_string(),
                "bad>data".to_string(),
            )
            .unwrap_err(),
            ParserCreatedProcessingInstructionError::DataContainsGreaterThan
        );
    }
}
