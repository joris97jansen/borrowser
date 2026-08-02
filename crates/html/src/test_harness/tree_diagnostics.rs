//! Non-canonical one-way tree-diagnostic projection for legacy DOM goldens.

use crate::html5::shared::{
    DocumentParseContext, ErrorPolicy, Input, ParserObservationConfig, ParserStage,
    SurfaceCaptureRequest,
};

/// The legacy DOM fixture corpus is intentionally small. A finite capacity
/// keeps this adapter bounded while leaving ample headroom for malformed
/// fixtures; exhaustion is a hard harness error rather than partial output.
const GOLDEN_TREE_PARSE_ERROR_CAPACITY: usize = 4_096;

pub struct TreeDiagnosticProjection;

impl TreeDiagnosticProjection {
    pub fn new_context() -> DocumentParseContext {
        DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::NotRequested,
                parse_errors: SurfaceCaptureRequest::Capture {
                    capacity: GOLDEN_TREE_PARSE_ERROR_CAPACITY,
                },
                implementation_diagnostics: SurfaceCaptureRequest::NotRequested,
                tree_transitions: SurfaceCaptureRequest::NotRequested,
                unsupported_features: SurfaceCaptureRequest::NotRequested,
            },
        )
    }

    pub fn push_str(context: &mut DocumentParseContext, input: &mut Input, text: &str) {
        input.push_str_observed(text, context.observation_position_index_mut());
    }

    pub fn finish_input(context: &mut DocumentParseContext, input: &mut Input) {
        let _ = input.finish_preprocessing_observed(context.observation_position_index_mut());
    }

    pub fn finish(context: &mut DocumentParseContext) -> Result<Vec<&'static str>, String> {
        let capture = context
            .take_observations()
            .ok_or_else(|| "tree diagnostic observation recorder was not installed".to_string())?;
        if let Some(failure) = capture.failure {
            return Err(format!("tree diagnostic observation failed: {failure:?}"));
        }
        if !capture.parse_errors.requested {
            return Err("tree parse-error capture was not requested".to_string());
        }
        if capture.parse_errors.dropped != 0 {
            return Err(format!(
                "tree parse-error capture exceeded finite capacity {GOLDEN_TREE_PARSE_ERROR_CAPACITY}: dropped={}",
                capture.parse_errors.dropped
            ));
        }
        Ok(capture
            .parse_errors
            .items
            .into_iter()
            .filter(|event| event.stage == ParserStage::TreeConstruction)
            .filter_map(|event| event.description)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{GOLDEN_TREE_PARSE_ERROR_CAPACITY, TreeDiagnosticProjection};
    use crate::html5::shared::{DocumentParseContext, Input, ParseErrorCode, WhatwgParseErrorCode};

    #[test]
    fn golden_projection_requires_explicit_observation_setup() {
        let mut ordinary = DocumentParseContext::new();
        assert!(
            TreeDiagnosticProjection::finish(&mut ordinary)
                .expect_err("ordinary contexts are not silently observed")
                .contains("was not installed")
        );
    }

    #[test]
    fn golden_projection_rejects_storage_drops_instead_of_projecting_a_prefix() {
        let mut context = TreeDiagnosticProjection::new_context();
        let mut input = Input::new();
        TreeDiagnosticProjection::push_str(&mut context, &mut input, "x");
        for _ in 0..=GOLDEN_TREE_PARSE_ERROR_CAPACITY {
            context.record_tokenizer_parse_error(
                &input,
                ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedNullCharacter),
                0,
                None,
                Some("adapter-capacity-test"),
                None,
            );
        }
        assert!(
            TreeDiagnosticProjection::finish(&mut context)
                .expect_err("partial canonical capture must fail")
                .contains("exceeded finite capacity")
        );
    }
}
