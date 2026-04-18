use super::convert::map_structure_error;
use super::spans::{component_list_span, span_from_bounds};
use super::{
    Combinator, CombinedSelector, ComplexSelector, CompoundSelector, CssComponentValue, CssInput,
    CssSpan, InvalidSelectorReason, SubclassSelector, TypeSelector, UnsupportedSelectorFeature,
};

pub(super) enum SegmentParseResult {
    Parsed(ComplexSelector),
    Unsupported(Vec<UnsupportedSelectorFeature>),
    Invalid {
        span: Option<CssSpan>,
        reason: InvalidSelectorReason,
    },
}

pub(super) enum SegmentParseError {
    Invalid {
        span: Option<CssSpan>,
        reason: InvalidSelectorReason,
    },
}

pub(super) struct SegmentParser<'a> {
    pub(super) input: &'a CssInput,
    pub(super) values: &'a [CssComponentValue],
    pub(super) index: usize,
    pub(super) unsupported: Vec<UnsupportedSelectorFeature>,
}

pub(super) struct TriviaRun {
    pub(super) saw_whitespace: bool,
    pub(super) first_span: Option<CssSpan>,
}

pub(super) struct ParsedCompound {
    pub(super) span: CssSpan,
    pub(super) supported: Option<CompoundSelector>,
}

pub(super) enum ParsedSimpleSelector {
    Type {
        span: CssSpan,
        selector: TypeSelector,
    },
    Subclass {
        span: CssSpan,
        selector: SubclassSelector,
    },
    Unsupported {
        span: CssSpan,
        features: Vec<UnsupportedSelectorFeature>,
    },
}

pub(super) enum ExplicitCombinator {
    Supported {
        combinator: Combinator,
        span: CssSpan,
    },
    Unsupported {
        feature: UnsupportedSelectorFeature,
        span: CssSpan,
    },
}

impl From<SegmentParseError> for SegmentParseResult {
    fn from(error: SegmentParseError) -> Self {
        match error {
            SegmentParseError::Invalid { span, reason } => Self::Invalid { span, reason },
        }
    }
}

impl<'a> SegmentParser<'a> {
    pub(super) fn new(input: &'a CssInput, values: &'a [CssComponentValue]) -> Self {
        Self {
            input,
            values,
            index: 0,
            unsupported: Vec::new(),
        }
    }

    pub(super) fn parse(mut self, fallback_span: Option<CssSpan>) -> SegmentParseResult {
        let segment_span = component_list_span(self.values).or(fallback_span);
        let leading_trivia = self.skip_trivia();

        if self.is_eof() {
            return SegmentParseResult::Invalid {
                span: leading_trivia.first_span.or(segment_span),
                reason: InvalidSelectorReason::EmptyCompoundSelector,
            };
        }

        if self.current_is_combinator() {
            return SegmentParseResult::Invalid {
                span: self.current_span().or(segment_span),
                reason: InvalidSelectorReason::LeadingCombinator,
            };
        }

        let head = match self.parse_compound() {
            Ok(compound) => compound,
            Err(error) => return error.into(),
        };

        let mut tail = Vec::new();

        loop {
            let trivia = self.skip_trivia();
            if self.is_eof() {
                break;
            }

            let mut combinator_start = trivia.first_span.or(self.current_span()).or(segment_span);
            let explicit = self.consume_explicit_combinator();
            let combinator = match explicit {
                Some(ExplicitCombinator::Supported { combinator, span }) => {
                    combinator_start = Some(span);
                    Some(combinator)
                }
                Some(ExplicitCombinator::Unsupported { feature, span }) => {
                    combinator_start = Some(span);
                    self.push_unsupported(feature);
                    None
                }
                None if trivia.saw_whitespace => Some(Combinator::Descendant),
                None => {
                    return SegmentParseResult::Invalid {
                        span: self.current_span().or(segment_span),
                        reason: InvalidSelectorReason::UnexpectedComponentValue,
                    };
                }
            };

            self.skip_trivia();

            if self.is_eof() {
                return SegmentParseResult::Invalid {
                    span: combinator_start,
                    reason: InvalidSelectorReason::TrailingCombinator,
                };
            }

            if self.current_is_combinator() {
                return SegmentParseResult::Invalid {
                    span: self.current_span().or(segment_span),
                    reason: InvalidSelectorReason::RepeatedCombinator,
                };
            }

            let next = match self.parse_compound() {
                Ok(compound) => compound,
                Err(SegmentParseError::Invalid {
                    span,
                    reason: InvalidSelectorReason::EmptyCompoundSelector,
                }) => {
                    return SegmentParseResult::Invalid {
                        span: span.or(combinator_start),
                        reason: InvalidSelectorReason::TrailingCombinator,
                    };
                }
                Err(error) => return error.into(),
            };

            if self.unsupported.is_empty()
                && let (Some(combinator), Some(selector)) = (combinator, next.supported)
            {
                let combined_span = span_from_bounds(
                    combinator_start.expect("combined selector start span"),
                    selector.span(),
                )
                .expect("combined selector span");
                match CombinedSelector::new(combined_span, combinator, selector) {
                    Ok(combined) => tail.push(combined),
                    Err(error) => {
                        return SegmentParseResult::Invalid {
                            span: Some(combined_span),
                            reason: map_structure_error(error),
                        };
                    }
                }
            }
        }

        if !self.unsupported.is_empty() {
            return SegmentParseResult::Unsupported(self.unsupported);
        }

        let selector_end = tail.last().map(CombinedSelector::span).unwrap_or(head.span);
        let selector_span = span_from_bounds(head.span, selector_end).expect("selector span");

        match ComplexSelector::new(
            selector_span,
            head.supported.expect("supported head compound"),
            tail,
        ) {
            Ok(selector) => SegmentParseResult::Parsed(selector),
            Err(error) => SegmentParseResult::Invalid {
                span: segment_span,
                reason: map_structure_error(error),
            },
        }
    }

    fn parse_compound(&mut self) -> Result<ParsedCompound, SegmentParseError> {
        let mut type_selector = None;
        let mut subclasses = Vec::new();
        let mut saw_simple = false;
        let mut compound_start = None;
        let mut compound_end = None;
        let mut compound_has_unsupported = false;

        loop {
            self.skip_comments();
            if self.is_eof()
                || self.current_is_whitespace()
                || self.current_is_combinator()
                || self.current_is_comma()
            {
                break;
            }

            let parsed = self.parse_simple_selector()?;

            let parsed_span = parsed.span();
            if compound_start.is_none() {
                compound_start = Some(parsed_span);
            }
            compound_end = Some(parsed_span);
            saw_simple = true;

            match parsed {
                ParsedSimpleSelector::Type { selector, .. } => {
                    if type_selector.is_some() || !subclasses.is_empty() {
                        return Err(SegmentParseError::Invalid {
                            span: Some(parsed_span),
                            reason: InvalidSelectorReason::MultipleTypeSelectors,
                        });
                    }
                    type_selector = Some(selector);
                }
                ParsedSimpleSelector::Subclass { selector, .. } => subclasses.push(selector),
                ParsedSimpleSelector::Unsupported { features, .. } => {
                    compound_has_unsupported = true;
                    for feature in features {
                        self.push_unsupported(feature);
                    }
                }
            }
        }

        if !saw_simple {
            return Err(SegmentParseError::Invalid {
                span: self.current_span(),
                reason: InvalidSelectorReason::EmptyCompoundSelector,
            });
        }

        let span = span_from_bounds(
            compound_start.expect("compound start"),
            compound_end.expect("compound end"),
        )
        .expect("compound span");

        if compound_has_unsupported {
            return Ok(ParsedCompound {
                span,
                supported: None,
            });
        }

        match CompoundSelector::new(span, type_selector, subclasses) {
            Ok(selector) => Ok(ParsedCompound {
                span,
                supported: Some(selector),
            }),
            Err(error) => Err(SegmentParseError::Invalid {
                span: Some(span),
                reason: map_structure_error(error),
            }),
        }
    }

    fn consume_explicit_combinator(&mut self) -> Option<ExplicitCombinator> {
        let token = self.current_token()?;

        let combinator = match token.kind {
            super::CssTokenKind::Delim('>') => ExplicitCombinator::Supported {
                combinator: Combinator::Child,
                span: token.span,
            },
            super::CssTokenKind::Delim('+') => ExplicitCombinator::Supported {
                combinator: Combinator::NextSibling,
                span: token.span,
            },
            super::CssTokenKind::Delim('~') => ExplicitCombinator::Supported {
                combinator: Combinator::SubsequentSibling,
                span: token.span,
            },
            super::CssTokenKind::Column => ExplicitCombinator::Unsupported {
                feature: UnsupportedSelectorFeature::ColumnCombinator,
                span: token.span,
            },
            _ => return None,
        };

        self.index += 1;
        Some(combinator)
    }

    fn push_unsupported(&mut self, feature: UnsupportedSelectorFeature) {
        if !self.unsupported.contains(&feature) {
            self.unsupported.push(feature);
        }
    }
}

impl ParsedSimpleSelector {
    fn span(&self) -> CssSpan {
        match self {
            Self::Type { span, .. }
            | Self::Subclass { span, .. }
            | Self::Unsupported { span, .. } => *span,
        }
    }
}
