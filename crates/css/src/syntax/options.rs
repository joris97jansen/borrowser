/// Parsing origin for diagnostics and entry-point-specific limit handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssParseOrigin {
    Stylesheet,
    StyleAttribute,
}

/// Recovery policy for malformed CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryPolicy {
    /// Malformed input is skipped using fixed structural boundaries and without
    /// implementation-defined heuristics.
    Deterministic,
}

/// Resource limits for bounded parser behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxLimits {
    pub max_stylesheet_input_bytes: usize,
    pub max_declaration_list_input_bytes: usize,
    pub max_lexical_tokens: usize,
    pub max_rules: usize,
    pub max_selectors_per_rule: usize,
    pub max_declarations_per_rule: usize,
    pub max_component_nesting_depth: usize,
    pub max_diagnostics: usize,
}

impl Default for SyntaxLimits {
    fn default() -> Self {
        Self {
            max_stylesheet_input_bytes: 4 * 1024 * 1024,
            max_declaration_list_input_bytes: 64 * 1024,
            max_lexical_tokens: 262_144,
            max_rules: 16_384,
            max_selectors_per_rule: 256,
            max_declarations_per_rule: 1_024,
            max_component_nesting_depth: 256,
            max_diagnostics: 128,
        }
    }
}

/// Options shared by stylesheet and declaration-list entry points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseOptions {
    pub origin: CssParseOrigin,
    pub recovery_policy: RecoveryPolicy,
    pub limits: SyntaxLimits,
    pub collect_diagnostics: bool,
}

impl ParseOptions {
    pub fn stylesheet() -> Self {
        Self {
            origin: CssParseOrigin::Stylesheet,
            recovery_policy: RecoveryPolicy::Deterministic,
            limits: SyntaxLimits::default(),
            collect_diagnostics: true,
        }
    }

    pub fn style_attribute() -> Self {
        Self {
            origin: CssParseOrigin::StyleAttribute,
            ..Self::stylesheet()
        }
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self::stylesheet()
    }
}
