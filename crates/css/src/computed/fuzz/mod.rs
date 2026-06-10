use crate::fuzz_support::{
    decode_bytes_lossy_unbounded, digest_snapshot, mix_str, mix_u64, mix_usize,
    synthesized_value_cases, truncate_string_to_char_boundary,
};
use crate::model::{self, DeclarationValue, serialize_value_for_snapshot};
use crate::properties::{PropertyComputedValueKind, PropertyId};
use crate::specified::{SpecifiedValueLimits, parse_specified_value_with_limits};
use crate::syntax::{ParseOptions, SyntaxLimits};

use super::normalize_specified_value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssValueFuzzConfig {
    pub seed: u64,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_property_cases: usize,
    pub max_value_cases_per_property: usize,
    pub syntax_limits: SyntaxLimits,
    pub specified_value_limits: SpecifiedValueLimits,
}

impl Default for CssValueFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x43_53_53_56_41_4c_46_5a,
            max_input_bytes: 64 * 1024,
            max_decoded_bytes: 256 * 1024,
            max_property_cases: PropertyId::ALL.len(),
            max_value_cases_per_property: 3,
            syntax_limits: SyntaxLimits::default(),
            specified_value_limits: SpecifiedValueLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssValueFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssValueFuzzSummary {
    pub seed: u64,
    pub termination: CssValueFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub properties_observed: usize,
    pub value_cases_observed: usize,
    pub missing_declaration_value_cases: usize,
    pub specified_ok_cases: usize,
    pub specified_error_cases: usize,
    pub computed_ok_cases: usize,
    pub computed_error_cases: usize,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssValueFuzzError {
    NonDeterministicValueObservation {
        property: &'static str,
        authored_value: String,
    },
    PropertyMismatch {
        property: &'static str,
        authored_value: String,
        actual: &'static str,
    },
    SpecifiedKindMismatch {
        property: &'static str,
        authored_value: String,
        actual: &'static str,
        expected: &'static str,
    },
    ComputedKindMismatch {
        property: &'static str,
        authored_value: String,
        actual: &'static str,
        expected: &'static str,
    },
}

impl std::fmt::Display for CssValueFuzzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonDeterministicValueObservation {
                property,
                authored_value,
            } => write!(
                f,
                "value fuzz observation for property '{}' and value {:?} was non-deterministic",
                property, authored_value
            ),
            Self::PropertyMismatch {
                property,
                authored_value,
                actual,
            } => write!(
                f,
                "specified parser for property '{}' and value {:?} returned property '{}'",
                property, authored_value, actual
            ),
            Self::SpecifiedKindMismatch {
                property,
                authored_value,
                actual,
                expected,
            } => write!(
                f,
                "specified parser for property '{}' and value {:?} returned kind '{}' instead of '{}'",
                property, authored_value, actual, expected
            ),
            Self::ComputedKindMismatch {
                property,
                authored_value,
                actual,
                expected,
            } => write!(
                f,
                "computed normalization for property '{}' and value {:?} returned kind '{}' instead of '{}'",
                property, authored_value, actual, expected
            ),
        }
    }
}

impl std::error::Error for CssValueFuzzError {}

pub fn run_seeded_value_fuzz_case(
    bytes: &[u8],
    config: CssValueFuzzConfig,
) -> Result<CssValueFuzzSummary, CssValueFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(CssValueFuzzSummary {
            seed: config.seed,
            termination: CssValueFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            properties_observed: 0,
            value_cases_observed: 0,
            missing_declaration_value_cases: 0,
            specified_ok_cases: 0,
            specified_error_cases: 0,
            computed_ok_cases: 0,
            computed_error_cases: 0,
            digest: 0,
        });
    }

    let decoded = decode_bytes_lossy_unbounded(bytes);
    if decoded.len() > config.max_decoded_bytes {
        return Ok(CssValueFuzzSummary {
            seed: config.seed,
            termination: CssValueFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            properties_observed: 0,
            value_cases_observed: 0,
            missing_declaration_value_cases: 0,
            specified_ok_cases: 0,
            specified_error_cases: 0,
            computed_ok_cases: 0,
            computed_error_cases: 0,
            digest: 0,
        });
    }

    let raw_value = truncate_string_to_char_boundary(decoded, config.max_decoded_bytes);
    let mut digest = mix_u64(0, config.seed);
    digest = mix_usize(digest, bytes.len());
    digest = mix_usize(digest, raw_value.len());

    let mut summary = CssValueFuzzSummary {
        seed: config.seed,
        termination: CssValueFuzzTermination::Completed,
        input_bytes: bytes.len(),
        decoded_bytes: raw_value.len(),
        properties_observed: 0,
        value_cases_observed: 0,
        missing_declaration_value_cases: 0,
        specified_ok_cases: 0,
        specified_error_cases: 0,
        computed_ok_cases: 0,
        computed_error_cases: 0,
        digest: 0,
    };

    let property_offset = (config.seed as usize) % PropertyId::ALL.len();
    let property_budget = config.max_property_cases.min(PropertyId::ALL.len());

    for property_index in 0..property_budget {
        let property = PropertyId::ALL[(property_offset + property_index) % PropertyId::ALL.len()];
        let cases = synthesized_value_cases(
            property,
            &raw_value,
            config.seed ^ property.as_index() as u64,
        );
        summary.properties_observed += 1;
        for value_case in cases.into_iter().take(config.max_value_cases_per_property) {
            let observation = observe_value_case(
                property,
                &value_case,
                &config.syntax_limits,
                &config.specified_value_limits,
            )?;
            summary.value_cases_observed += 1;
            digest = mix_str(digest, property.name());
            digest = mix_str(digest, &value_case);
            digest = mix_u64(
                digest,
                digest_snapshot(config.seed, std::slice::from_ref(&observation.snapshot)),
            );
            accumulate_value_observation(&mut summary, &observation);
        }
    }

    summary.digest = digest;
    Ok(summary)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValueCaseObservation {
    snapshot: String,
    specified_outcome: SpecifiedOutcome,
    computed_outcome: ComputedOutcome,
    missing_declaration_value: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SpecifiedOutcome {
    NotParsed,
    Ok {
        kind: &'static str,
        css_text: String,
    },
    Error {
        kind: &'static str,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ComputedOutcome {
    NotComputed,
    Ok {
        kind: &'static str,
        debug_label: String,
    },
    Error {
        kind: &'static str,
    },
}

fn observe_value_case(
    property: PropertyId,
    value_text: &str,
    syntax_limits: &SyntaxLimits,
    specified_limits: &SpecifiedValueLimits,
) -> Result<ValueCaseObservation, CssValueFuzzError> {
    let first = observe_value_case_once(property, value_text, syntax_limits, specified_limits)?;
    let second = observe_value_case_once(property, value_text, syntax_limits, specified_limits)?;
    if first != second {
        return Err(CssValueFuzzError::NonDeterministicValueObservation {
            property: property.name(),
            authored_value: value_text.to_string(),
        });
    }
    Ok(first)
}

fn observe_value_case_once(
    property: PropertyId,
    value_text: &str,
    syntax_limits: &SyntaxLimits,
    specified_limits: &SpecifiedValueLimits,
) -> Result<ValueCaseObservation, CssValueFuzzError> {
    let stylesheet_source = format!("a {{ {}: {}; }}", property.name(), value_text);
    let parse = model::parse_stylesheet_with_options(
        &stylesheet_source,
        &ParseOptions {
            limits: syntax_limits.clone(),
            ..ParseOptions::stylesheet()
        },
    );

    let mut out = String::new();
    out.push_str("version: 1\n");
    out.push_str("computed-value\n");
    out.push_str(&format!("property: {}\n", property.name()));
    out.push_str(&format!(
        "specified-contract: {}\n",
        property.metadata().specified_value.as_debug_label()
    ));
    out.push_str(&format!(
        "computed-contract: {}\n",
        property.metadata().computed_value.as_debug_label()
    ));
    out.push_str(&format!("authored-value: {value_text}\n"));
    out.push_str(&format!(
        "syntax-diagnostics: {}\n",
        parse.diagnostics.len()
    ));
    out.push_str("model-parse:\n");
    for line in parse.to_debug_snapshot().lines() {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }

    let maybe_value = first_declaration_value(&parse);
    let Some(value) = maybe_value else {
        out.push_str("model: missing-declaration-value\n");
        out.push_str("specified: not-parsed\n");
        out.push_str("computed: not-computed\n");
        return Ok(ValueCaseObservation {
            snapshot: out,
            specified_outcome: SpecifiedOutcome::NotParsed,
            computed_outcome: ComputedOutcome::NotComputed,
            missing_declaration_value: true,
        });
    };

    out.push_str("model-value:\n");
    for line in serialize_value_for_snapshot(&parse.input, value).lines() {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }

    let specified_value = match parse_specified_value_with_limits(property, value, specified_limits)
    {
        Ok(specified) => {
            if specified.property() != property {
                return Err(CssValueFuzzError::PropertyMismatch {
                    property: property.name(),
                    authored_value: value_text.to_string(),
                    actual: specified.property().name(),
                });
            }
            let actual = specified.kind().as_debug_label();
            let expected = property.metadata().specified_value.as_debug_label();
            if actual != expected {
                return Err(CssValueFuzzError::SpecifiedKindMismatch {
                    property: property.name(),
                    authored_value: value_text.to_string(),
                    actual,
                    expected,
                });
            }
            out.push_str(&format!("specified-kind: {}\n", actual));
            out.push_str(&format!("specified: {}\n", specified.to_css_text()));
            specified
        }
        Err(error) => {
            let kind = error.kind().as_debug_label();
            out.push_str(&format!("specified-error: {kind}\n"));
            out.push_str("computed: not-computed\n");
            return Ok(ValueCaseObservation {
                snapshot: out,
                specified_outcome: SpecifiedOutcome::Error { kind },
                computed_outcome: ComputedOutcome::NotComputed,
                missing_declaration_value: false,
            });
        }
    };

    let specified = SpecifiedOutcome::Ok {
        kind: specified_value.kind().as_debug_label(),
        css_text: specified_value.to_css_text(),
    };
    let computed_outcome = match normalize_specified_value(&specified_value) {
        Ok(computed) => {
            let actual = computed.discriminant().as_debug_label();
            let expected = property_computed_kind_label(property.metadata().computed_value);
            if actual != expected {
                return Err(CssValueFuzzError::ComputedKindMismatch {
                    property: property.name(),
                    authored_value: value_text.to_string(),
                    actual,
                    expected,
                });
            }
            out.push_str(&format!("computed-kind: {}\n", actual));
            out.push_str(&format!("computed: {}\n", computed.to_debug_label()));
            ComputedOutcome::Ok {
                kind: actual,
                debug_label: computed.to_debug_label(),
            }
        }
        Err(error) => {
            let kind = error.kind().as_debug_label();
            out.push_str(&format!("computed-error: {kind}\n"));
            ComputedOutcome::Error { kind }
        }
    };

    Ok(ValueCaseObservation {
        snapshot: out,
        specified_outcome: specified,
        computed_outcome,
        missing_declaration_value: false,
    })
}

fn property_computed_kind_label(kind: PropertyComputedValueKind) -> &'static str {
    match kind {
        PropertyComputedValueKind::AbsoluteColor => "color",
        PropertyComputedValueKind::BorderStyleKeyword => "border-style",
        PropertyComputedValueKind::DisplayKeyword => "display",
        PropertyComputedValueKind::OverflowKeyword => "overflow",
        PropertyComputedValueKind::PositionKeyword => "position",
        PropertyComputedValueKind::AbsoluteLength => "length",
        PropertyComputedValueKind::LengthPercentageOrAuto => "length-percentage-or-auto",
        PropertyComputedValueKind::LengthPercentageOrNone => "length-percentage-or-none",
    }
}

fn accumulate_value_observation(
    summary: &mut CssValueFuzzSummary,
    observation: &ValueCaseObservation,
) {
    if observation.missing_declaration_value {
        summary.missing_declaration_value_cases += 1;
    }
    match &observation.specified_outcome {
        SpecifiedOutcome::NotParsed => {}
        SpecifiedOutcome::Ok { .. } => summary.specified_ok_cases += 1,
        SpecifiedOutcome::Error { .. } => summary.specified_error_cases += 1,
    }
    match &observation.computed_outcome {
        ComputedOutcome::NotComputed => {}
        ComputedOutcome::Ok { .. } => summary.computed_ok_cases += 1,
        ComputedOutcome::Error { .. } => summary.computed_error_cases += 1,
    }
}

fn first_declaration_value(parse: &model::StylesheetParse) -> Option<&DeclarationValue> {
    parse.stylesheet.rules.iter().find_map(|rule| match rule {
        model::Rule::Style(rule) => rule
            .declarations
            .declarations
            .first()
            .map(|decl| &decl.value),
        model::Rule::At(_) => None,
    })
}

#[cfg(test)]
mod tests;
