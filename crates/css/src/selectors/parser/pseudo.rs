use super::segment::{ParsedSimpleSelector, SegmentParseError, SegmentParser};
use super::spans::span_from_bounds;
use super::{
    CssComponentValue, CssSpan, CssToken, CssTokenKind, InvalidSelectorReason, SubclassSelector,
    TreeStructuralPseudoClass, TreeStructuralPseudoClassSelector, UnsupportedSelectorFeature,
};

impl<'a> SegmentParser<'a> {
    pub(super) fn parse_pseudo_selector(
        &mut self,
        first_colon_span: CssSpan,
    ) -> Result<ParsedSimpleSelector, SegmentParseError> {
        self.index += 1;
        self.skip_comments();

        let is_double_colon = matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Colon,
                ..
            }))
        );
        if is_double_colon {
            self.index += 1;
            self.skip_comments();
        }

        match self.current_value() {
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Ident(name),
                span: name_span,
            })) => {
                let selector_span = span_from_bounds(first_colon_span, *name_span).ok_or(
                    SegmentParseError::Invalid {
                        span: Some(first_colon_span),
                        reason: InvalidSelectorReason::InvariantViolation,
                    },
                )?;
                let resolved_name = name.resolve(self.input).ok_or(SegmentParseError::Invalid {
                    span: Some(*name_span),
                    reason: InvalidSelectorReason::UnexpectedComponentValue,
                })?;
                let classification = classify_identifier_pseudo(&resolved_name, is_double_colon);
                self.index += 1;

                match classification {
                    IdentifierPseudoClassification::Supported(pseudo_class) => {
                        Ok(ParsedSimpleSelector::Subclass {
                            span: selector_span,
                            selector: SubclassSelector::TreeStructuralPseudoClass(
                                TreeStructuralPseudoClassSelector::new(selector_span, pseudo_class),
                            ),
                        })
                    }
                    IdentifierPseudoClassification::Unsupported(feature) => {
                        Ok(ParsedSimpleSelector::Unsupported {
                            span: selector_span,
                            features: vec![feature],
                        })
                    }
                }
            }
            Some(CssComponentValue::Function(function)) => {
                let selector_span = span_from_bounds(first_colon_span, function.span).ok_or(
                    SegmentParseError::Invalid {
                        span: Some(first_colon_span),
                        reason: InvalidSelectorReason::InvariantViolation,
                    },
                )?;
                let resolved_name =
                    function
                        .name
                        .resolve(self.input)
                        .ok_or(SegmentParseError::Invalid {
                            span: Some(function.span),
                            reason: InvalidSelectorReason::UnexpectedComponentValue,
                        })?;
                let features = classify_functional_pseudo(&resolved_name, is_double_colon);
                self.index += 1;
                Ok(ParsedSimpleSelector::Unsupported {
                    span: selector_span,
                    features,
                })
            }
            _ => Err(SegmentParseError::Invalid {
                span: Some(first_colon_span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdentifierPseudoClassification {
    Supported(TreeStructuralPseudoClass),
    Unsupported(UnsupportedSelectorFeature),
}

fn classify_identifier_pseudo(name: &str, is_double_colon: bool) -> IdentifierPseudoClassification {
    if is_double_colon {
        return IdentifierPseudoClassification::Unsupported(
            UnsupportedSelectorFeature::PseudoElement,
        );
    }

    if is_legacy_single_colon_pseudo_element(name) {
        return IdentifierPseudoClassification::Unsupported(
            UnsupportedSelectorFeature::PseudoElement,
        );
    }
    TreeStructuralPseudoClass::from_css_keyword(name).map_or(
        IdentifierPseudoClassification::Unsupported(UnsupportedSelectorFeature::PseudoClass),
        IdentifierPseudoClassification::Supported,
    )
}

fn classify_functional_pseudo(
    name: &str,
    is_double_colon: bool,
) -> Vec<UnsupportedSelectorFeature> {
    if is_double_colon {
        return vec![UnsupportedSelectorFeature::PseudoElement];
    }

    let mut features = vec![UnsupportedSelectorFeature::FunctionalPseudoClass];
    if name.eq_ignore_ascii_case("is") || name.eq_ignore_ascii_case("where") {
        features.push(UnsupportedSelectorFeature::ForgivingSelectorList);
    }
    features
}

fn is_legacy_single_colon_pseudo_element(name: &str) -> bool {
    name.eq_ignore_ascii_case("before")
        || name.eq_ignore_ascii_case("after")
        || name.eq_ignore_ascii_case("first-line")
        || name.eq_ignore_ascii_case("first-letter")
}
