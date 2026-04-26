use crate::computed::{ComputedStyleResolutionError, compute_document_styles_from_resolved_styles};
use crate::fuzz_support::{
    DomFuzzLimits, decode_bytes_lossy_unbounded, digest_snapshot, mix_str, mix_u64, mix_usize,
    synthesize_dom_from_bytes, synthesized_supported_stylesheet_suite,
};
use crate::model;
use crate::syntax::{ParseOptions, SyntaxLimits};

use super::{StyleResolutionError, StyleResolutionLimits, try_resolve_document_styles_with_limits};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssCascadeFuzzConfig {
    pub seed: u64,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_stylesheet_cases: usize,
    pub max_resolved_elements_observed: usize,
    pub max_computed_elements_observed: usize,
    pub syntax_limits: SyntaxLimits,
    pub style_resolution_limits: StyleResolutionLimits,
    pub dom_limits: DomFuzzLimits,
}

impl Default for CssCascadeFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x43_53_53_43_41_53_46_5a,
            max_input_bytes: 64 * 1024,
            max_decoded_bytes: 256 * 1024,
            max_stylesheet_cases: 2,
            max_resolved_elements_observed: 1_024,
            max_computed_elements_observed: 1_024,
            syntax_limits: SyntaxLimits::default(),
            style_resolution_limits: StyleResolutionLimits::default(),
            dom_limits: DomFuzzLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssCascadeFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
    RejectedMaxStylesheetCases,
    RejectedMaxResolvedElementsObserved,
    RejectedMaxComputedElementsObserved,
    StyleResolutionLimitExceeded,
    SelectorMatchingLimitExceeded,
    ComputedNormalizationError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssCascadeFuzzSummary {
    pub seed: u64,
    pub termination: CssCascadeFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub stylesheet_cases_observed: usize,
    pub resolved_elements_observed: usize,
    pub computed_elements_observed: usize,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssCascadeFuzzError {
    StyleResolutionInvariant { detail: String },
    ComputedStyleInvariant { detail: String },
}

impl std::fmt::Display for CssCascadeFuzzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StyleResolutionInvariant { detail } => {
                write!(
                    f,
                    "cascade fuzz style-resolution invariant failed: {detail}"
                )
            }
            Self::ComputedStyleInvariant { detail } => {
                write!(f, "cascade fuzz computed-style invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for CssCascadeFuzzError {}

pub fn run_seeded_cascade_fuzz_case(
    bytes: &[u8],
    config: CssCascadeFuzzConfig,
) -> Result<CssCascadeFuzzSummary, CssCascadeFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(CssCascadeFuzzSummary {
            seed: config.seed,
            termination: CssCascadeFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            stylesheet_cases_observed: 0,
            resolved_elements_observed: 0,
            computed_elements_observed: 0,
            digest: 0,
        });
    }

    let decoded = decode_bytes_lossy_unbounded(bytes);
    if decoded.len() > config.max_decoded_bytes {
        return Ok(CssCascadeFuzzSummary {
            seed: config.seed,
            termination: CssCascadeFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            stylesheet_cases_observed: 0,
            resolved_elements_observed: 0,
            computed_elements_observed: 0,
            digest: 0,
        });
    }

    let (dom, dom_summary) = synthesize_dom_from_bytes(bytes, &config.dom_limits);
    let stylesheet_cases = synthesized_supported_stylesheet_suite(bytes, &decoded);

    let mut digest = mix_u64(0, config.seed);
    digest = mix_usize(digest, bytes.len());
    digest = mix_usize(digest, decoded.len());
    digest = mix_usize(digest, dom_summary.element_count);
    digest = mix_usize(digest, dom_summary.inline_style_attributes);

    let mut parsed_stylesheets = Vec::new();
    let mut observed_stylesheets = 0usize;
    for stylesheet_source in stylesheet_cases {
        if observed_stylesheets >= config.max_stylesheet_cases {
            return Ok(CssCascadeFuzzSummary {
                seed: config.seed,
                termination: CssCascadeFuzzTermination::RejectedMaxStylesheetCases,
                input_bytes: bytes.len(),
                decoded_bytes: decoded.len(),
                stylesheet_cases_observed: observed_stylesheets,
                resolved_elements_observed: 0,
                computed_elements_observed: 0,
                digest,
            });
        }
        digest = mix_str(digest, &stylesheet_source);
        let parse_options = ParseOptions {
            limits: config.syntax_limits.clone(),
            ..ParseOptions::stylesheet()
        };
        let stylesheet = model::parse_stylesheet_with_options(&stylesheet_source, &parse_options);
        digest = mix_u64(
            digest,
            digest_snapshot(config.seed, &[stylesheet.to_debug_snapshot()]),
        );
        parsed_stylesheets.push(stylesheet);
        observed_stylesheets += 1;
    }

    let resolved = match try_resolve_document_styles_with_limits(
        &dom,
        &parsed_stylesheets,
        &config.style_resolution_limits,
    ) {
        Ok(resolved) => resolved,
        Err(StyleResolutionError::LimitExceeded { configured, .. }) => {
            digest = mix_usize(digest, configured);
            return Ok(CssCascadeFuzzSummary {
                seed: config.seed,
                termination: CssCascadeFuzzTermination::StyleResolutionLimitExceeded,
                input_bytes: bytes.len(),
                decoded_bytes: decoded.len(),
                stylesheet_cases_observed: observed_stylesheets,
                resolved_elements_observed: 0,
                computed_elements_observed: 0,
                digest,
            });
        }
        Err(StyleResolutionError::SelectorMatching(error)) => {
            digest = mix_str(digest, &error.to_string());
            return Ok(CssCascadeFuzzSummary {
                seed: config.seed,
                termination: CssCascadeFuzzTermination::SelectorMatchingLimitExceeded,
                input_bytes: bytes.len(),
                decoded_bytes: decoded.len(),
                stylesheet_cases_observed: observed_stylesheets,
                resolved_elements_observed: 0,
                computed_elements_observed: 0,
                digest,
            });
        }
        Err(error @ StyleResolutionError::UnsupportedConfiguration { .. })
        | Err(error @ StyleResolutionError::RuleInputBuild(_)) => {
            return Err(CssCascadeFuzzError::StyleResolutionInvariant {
                detail: error.to_string(),
            });
        }
    };

    let resolved_count = resolved.entries().len();
    if resolved_count > config.max_resolved_elements_observed {
        return Ok(CssCascadeFuzzSummary {
            seed: config.seed,
            termination: CssCascadeFuzzTermination::RejectedMaxResolvedElementsObserved,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            stylesheet_cases_observed: observed_stylesheets,
            resolved_elements_observed: resolved_count,
            computed_elements_observed: 0,
            digest: mix_u64(
                digest,
                digest_snapshot(config.seed, &[resolved.to_debug_snapshot()]),
            ),
        });
    }

    digest = mix_u64(
        digest,
        digest_snapshot(config.seed, &[resolved.to_debug_snapshot()]),
    );

    let computed = match compute_document_styles_from_resolved_styles(&dom, &resolved) {
        Ok(computed) => computed,
        Err(ComputedStyleResolutionError::Normalization(error)) => {
            digest = mix_str(digest, &error.to_string());
            return Ok(CssCascadeFuzzSummary {
                seed: config.seed,
                termination: CssCascadeFuzzTermination::ComputedNormalizationError,
                input_bytes: bytes.len(),
                decoded_bytes: decoded.len(),
                stylesheet_cases_observed: observed_stylesheets,
                resolved_elements_observed: resolved_count,
                computed_elements_observed: 0,
                digest,
            });
        }
        Err(error) => {
            return Err(CssCascadeFuzzError::ComputedStyleInvariant {
                detail: error.to_string(),
            });
        }
    };

    let computed_count = computed.entries().len();
    if computed_count > config.max_computed_elements_observed {
        return Ok(CssCascadeFuzzSummary {
            seed: config.seed,
            termination: CssCascadeFuzzTermination::RejectedMaxComputedElementsObserved,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            stylesheet_cases_observed: observed_stylesheets,
            resolved_elements_observed: resolved_count,
            computed_elements_observed: computed_count,
            digest: mix_u64(
                digest,
                digest_snapshot(config.seed, &[computed.to_debug_snapshot()]),
            ),
        });
    }

    digest = mix_u64(
        digest,
        digest_snapshot(config.seed, &[computed.to_debug_snapshot()]),
    );

    Ok(CssCascadeFuzzSummary {
        seed: config.seed,
        termination: CssCascadeFuzzTermination::Completed,
        input_bytes: bytes.len(),
        decoded_bytes: decoded.len(),
        stylesheet_cases_observed: observed_stylesheets,
        resolved_elements_observed: resolved_count,
        computed_elements_observed: computed_count,
        digest,
    })
}

#[cfg(test)]
mod tests;
