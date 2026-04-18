use super::convert::{map_structure_error, selector_ident_from_text};
use super::segment::{ParsedSimpleSelector, SegmentParseError, SegmentParser};
use super::spans::span_from_bounds;
use super::{
    ClassSelector, CssBlockKind, CssComponentValue, CssHashKind, CssInput, CssToken, CssTokenKind,
    CssTokenText, IdSelector, InvalidSelectorReason, SubclassSelector, TypeSelector,
    UnsupportedSelectorFeature,
};

impl<'a> SegmentParser<'a> {
    pub(super) fn parse_simple_selector(
        &mut self,
    ) -> Result<ParsedSimpleSelector, SegmentParseError> {
        let current = self
            .current_value()
            .expect("simple selector parse requires a current component");

        match current {
            CssComponentValue::PreservedToken(token) => self.parse_token_selector(token),
            CssComponentValue::SimpleBlock(block) if block.kind == CssBlockKind::Square => {
                self.index += 1;
                self.parse_attribute_selector(block.span, &block.value)
            }
            CssComponentValue::SimpleBlock(block) => Err(SegmentParseError::Invalid {
                span: Some(block.span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            }),
            CssComponentValue::Function(function) => Err(SegmentParseError::Invalid {
                span: Some(function.span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            }),
        }
    }

    pub(super) fn parse_token_selector(
        &mut self,
        token: &CssToken,
    ) -> Result<ParsedSimpleSelector, SegmentParseError> {
        match &token.kind {
            CssTokenKind::Ident(text) => {
                if self.next_non_comment_is_namespace_delim() {
                    let span = self.consume_namespace_sequence();
                    return Ok(ParsedSimpleSelector::Unsupported {
                        span,
                        features: vec![UnsupportedSelectorFeature::Namespace],
                    });
                }

                self.index += 1;
                let name = selector_ident_from_text(self.input, text)?;
                let selector = TypeSelector::named(token.span, name).map_err(|error| {
                    SegmentParseError::Invalid {
                        span: Some(token.span),
                        reason: map_structure_error(error),
                    }
                })?;
                Ok(ParsedSimpleSelector::Type {
                    span: token.span,
                    selector,
                })
            }
            CssTokenKind::Delim('*') => {
                if self.next_non_comment_is_namespace_delim() {
                    let span = self.consume_namespace_sequence();
                    return Ok(ParsedSimpleSelector::Unsupported {
                        span,
                        features: vec![UnsupportedSelectorFeature::Namespace],
                    });
                }

                self.index += 1;
                Ok(ParsedSimpleSelector::Type {
                    span: token.span,
                    selector: TypeSelector::universal(token.span),
                })
            }
            CssTokenKind::Hash {
                value,
                kind: CssHashKind::Id,
            } => {
                self.index += 1;
                let name = selector_ident_from_text(self.input, value)?;
                let selector = IdSelector::new(token.span, name).map_err(|error| {
                    SegmentParseError::Invalid {
                        span: Some(token.span),
                        reason: map_structure_error(error),
                    }
                })?;
                Ok(ParsedSimpleSelector::Subclass {
                    span: token.span,
                    selector: SubclassSelector::Id(selector),
                })
            }
            CssTokenKind::Hash { .. } => Err(SegmentParseError::Invalid {
                span: Some(token.span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            }),
            CssTokenKind::Delim('.') => self.parse_class_selector(token.span),
            CssTokenKind::Colon => self.parse_pseudo_selector(token.span),
            CssTokenKind::Delim('&') => {
                self.index += 1;
                Ok(ParsedSimpleSelector::Unsupported {
                    span: token.span,
                    features: vec![UnsupportedSelectorFeature::NestingSelector],
                })
            }
            CssTokenKind::Delim('|') => {
                let span = self.consume_namespace_sequence();
                Ok(ParsedSimpleSelector::Unsupported {
                    span,
                    features: vec![UnsupportedSelectorFeature::Namespace],
                })
            }
            _ => Err(SegmentParseError::Invalid {
                span: Some(token.span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            }),
        }
    }

    pub(super) fn parse_class_selector(
        &mut self,
        dot_span: super::CssSpan,
    ) -> Result<ParsedSimpleSelector, SegmentParseError> {
        self.index += 1;
        self.skip_comments();

        let Some(CssComponentValue::PreservedToken(token)) = self.current_value() else {
            return Err(SegmentParseError::Invalid {
                span: Some(dot_span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            });
        };

        let CssTokenKind::Ident(text) = &token.kind else {
            return Err(SegmentParseError::Invalid {
                span: Some(token.span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            });
        };

        let selector_span = span_from_bounds(dot_span, token.span).expect("class selector span");
        let name = selector_ident_from_text(self.input, text)?;
        let selector = ClassSelector::new(selector_span, name).map_err(|error| {
            SegmentParseError::Invalid {
                span: Some(selector_span),
                reason: map_structure_error(error),
            }
        })?;
        self.index += 1;

        Ok(ParsedSimpleSelector::Subclass {
            span: selector_span,
            selector: SubclassSelector::Class(selector),
        })
    }

    pub(super) fn parse_pseudo_selector(
        &mut self,
        first_colon_span: super::CssSpan,
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

        let mut features = Vec::new();
        if is_double_colon {
            self.index += 1;
            self.skip_comments();
            features.push(UnsupportedSelectorFeature::PseudoElement);
        }

        let end_span = match self.current_value() {
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Ident(_),
                span,
            })) => {
                self.index += 1;
                if !is_double_colon {
                    features.push(UnsupportedSelectorFeature::PseudoClass);
                }
                *span
            }
            Some(CssComponentValue::Function(function)) => {
                self.index += 1;
                if is_double_colon {
                    features.push(UnsupportedSelectorFeature::PseudoElement);
                } else {
                    features.push(UnsupportedSelectorFeature::FunctionalPseudoClass);
                    if function_name_is_forgiving_list(self.input, &function.name) {
                        features.push(UnsupportedSelectorFeature::ForgivingSelectorList);
                    }
                }
                function.span
            }
            _ => {
                return Err(SegmentParseError::Invalid {
                    span: Some(first_colon_span),
                    reason: InvalidSelectorReason::UnexpectedComponentValue,
                });
            }
        };

        Ok(ParsedSimpleSelector::Unsupported {
            span: span_from_bounds(first_colon_span, end_span).expect("pseudo selector span"),
            features,
        })
    }
}

fn function_name_is_forgiving_list(input: &CssInput, name: &CssTokenText) -> bool {
    matches!(name.resolve(input).as_deref(), Some("is") | Some("where"))
}
