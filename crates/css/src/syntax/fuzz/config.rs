use crate::syntax::SyntaxLimits;

const DEFAULT_MAX_INPUT_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_DECODED_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_TOKENS_OBSERVED: usize = 128 * 1024;
const DEFAULT_MAX_RULES_OBSERVED: usize = 16_384;
const DEFAULT_MAX_DECLARATIONS_OBSERVED: usize = 65_536;
const DEFAULT_MAX_COMPONENT_VALUES_OBSERVED: usize = 128 * 1024;
const DEFAULT_MAX_DIAGNOSTICS_OBSERVED: usize = 128;

pub fn derive_css_fuzz_seed(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^ ((bytes.len() as u64).rotate_left(17))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssTokenizerFuzzConfig {
    pub seed: u64,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_tokens_observed: usize,
    pub max_diagnostics_observed: usize,
}

impl Default for CssTokenizerFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x43_53_53_54_4f_4b_46_5a,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_decoded_bytes: DEFAULT_MAX_DECODED_BYTES,
            max_tokens_observed: DEFAULT_MAX_TOKENS_OBSERVED,
            max_diagnostics_observed: DEFAULT_MAX_DIAGNOSTICS_OBSERVED,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssTokenizerFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
    RejectedMaxTokensObserved,
    RejectedMaxDiagnosticsObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssTokenizerFuzzSummary {
    pub seed: u64,
    pub termination: CssTokenizerFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub tokens_observed: usize,
    pub diagnostics_observed: usize,
    pub hit_limit: bool,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssParserFuzzConfig {
    pub seed: u64,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_rules_observed: usize,
    pub max_declarations_observed: usize,
    pub max_component_values_observed: usize,
    pub max_diagnostics_observed: usize,
    pub syntax_limits: SyntaxLimits,
}

impl Default for CssParserFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x43_53_53_50_41_52_46_5a,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_decoded_bytes: DEFAULT_MAX_DECODED_BYTES,
            max_rules_observed: DEFAULT_MAX_RULES_OBSERVED,
            max_declarations_observed: DEFAULT_MAX_DECLARATIONS_OBSERVED,
            max_component_values_observed: DEFAULT_MAX_COMPONENT_VALUES_OBSERVED,
            max_diagnostics_observed: DEFAULT_MAX_DIAGNOSTICS_OBSERVED,
            syntax_limits: SyntaxLimits {
                max_stylesheet_input_bytes: DEFAULT_MAX_DECODED_BYTES,
                max_declaration_list_input_bytes: DEFAULT_MAX_DECODED_BYTES,
                max_lexical_tokens: 128 * 1024,
                max_diagnostics: DEFAULT_MAX_DIAGNOSTICS_OBSERVED,
                ..SyntaxLimits::default()
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssParserFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
    RejectedMaxRulesObserved,
    RejectedMaxDeclarationsObserved,
    RejectedMaxComponentValuesObserved,
    RejectedMaxDiagnosticsObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssParserFuzzSummary {
    pub seed: u64,
    pub termination: CssParserFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub rules_observed: usize,
    pub declarations_observed: usize,
    pub component_values_observed: usize,
    pub diagnostics_observed: usize,
    pub hit_limit: bool,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssSyntaxFuzzError {
    TokenStreamInvariantViolation {
        detail: String,
    },
    StructuralInvariantViolation {
        phase: &'static str,
        detail: String,
    },
    InvalidDiagnosticOffset {
        phase: &'static str,
        diagnostic_index: usize,
        byte_offset: usize,
        input_bytes: usize,
    },
}

impl std::fmt::Display for CssSyntaxFuzzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenStreamInvariantViolation { detail } => {
                write!(f, "token stream invariant violation: {detail}")
            }
            Self::StructuralInvariantViolation { phase, detail } => {
                write!(f, "{phase} structural invariant violation: {detail}")
            }
            Self::InvalidDiagnosticOffset {
                phase,
                diagnostic_index,
                byte_offset,
                input_bytes,
            } => write!(
                f,
                "{phase} emitted diagnostic #{diagnostic_index} at byte offset {byte_offset}, beyond input length {input_bytes}"
            ),
        }
    }
}

impl std::error::Error for CssSyntaxFuzzError {}
