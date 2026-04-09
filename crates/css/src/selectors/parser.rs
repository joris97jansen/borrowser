use super::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue, ClassSelector, Combinator, CombinedSelector, ComplexSelector, CompoundSelector,
    IdSelector, InvalidSelectorList, InvalidSelectorReason, SelectorIdent, SelectorList,
    SelectorListParseResult, SelectorString, SelectorStructureError, SubclassSelector,
    TypeSelector, UnsupportedSelectorFeature, UnsupportedSelectorList,
};
use crate::syntax::{
    CssBlockKind, CssComponentValue, CssInput, CssSpan, CssToken, CssTokenKind, CssTokenText,
};

pub fn parse_selector_list(
    input: &CssInput,
    values: &[CssComponentValue],
) -> SelectorListParseResult {
    let overall_span = component_list_span(values);
    let has_any_significant = values.iter().any(|value| !is_trivia_component(value));

    if !has_any_significant {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            overall_span,
            InvalidSelectorReason::EmptySelectorList,
        ));
    }

    let mut selectors = Vec::new();
    let mut unsupported = Vec::new();
    let mut segment_start = 0usize;

    for comma_index in top_level_comma_indices(values).chain(std::iter::once(values.len())) {
        let segment = &values[segment_start..comma_index];
        match parse_selector_segment(input, segment, overall_span) {
            SegmentParseResult::Parsed(selector) => selectors.push(selector),
            SegmentParseResult::Unsupported(features) => unsupported.extend(features),
            SegmentParseResult::Invalid { span, reason } => {
                return SelectorListParseResult::Invalid(InvalidSelectorList::new(span, reason));
            }
        }
        segment_start = comma_index.saturating_add(1);
    }

    if !unsupported.is_empty() {
        return SelectorListParseResult::Unsupported(UnsupportedSelectorList::from_features(
            overall_span,
            unsupported,
        ));
    }

    match SelectorList::new(overall_span, selectors) {
        Ok(list) => SelectorListParseResult::Parsed(list),
        Err(error) => SelectorListParseResult::Invalid(InvalidSelectorList::new(
            overall_span,
            map_structure_error(error),
        )),
    }
}

enum SegmentParseResult {
    Parsed(ComplexSelector),
    Unsupported(Vec<UnsupportedSelectorFeature>),
    Invalid {
        span: Option<CssSpan>,
        reason: InvalidSelectorReason,
    },
}

enum SegmentParseError {
    Invalid {
        span: Option<CssSpan>,
        reason: InvalidSelectorReason,
    },
}

struct SegmentParser<'a> {
    input: &'a CssInput,
    values: &'a [CssComponentValue],
    index: usize,
    unsupported: Vec<UnsupportedSelectorFeature>,
}

struct TriviaRun {
    saw_whitespace: bool,
    first_span: Option<CssSpan>,
}

struct ParsedCompound {
    span: CssSpan,
    supported: Option<CompoundSelector>,
}

enum ParsedSimpleSelector {
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

enum ExplicitCombinator {
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
    fn parse(mut self, fallback_span: Option<CssSpan>) -> SegmentParseResult {
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

    fn parse_simple_selector(&mut self) -> Result<ParsedSimpleSelector, SegmentParseError> {
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

    fn parse_token_selector(
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
                kind: crate::syntax::CssHashKind::Id,
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

    fn parse_class_selector(
        &mut self,
        dot_span: CssSpan,
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

    fn parse_pseudo_selector(
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

    fn parse_attribute_selector(
        &mut self,
        block_span: CssSpan,
        values: &[CssComponentValue],
    ) -> Result<ParsedSimpleSelector, SegmentParseError> {
        let mut index = 0usize;
        skip_attribute_trivia(values, &mut index);

        if index >= values.len() {
            return Err(SegmentParseError::Invalid {
                span: Some(block_span),
                reason: InvalidSelectorReason::MissingAttributeName,
            });
        }

        let (name, name_span, namespace_feature) =
            parse_attribute_name(self.input, values, &mut index)?;
        if namespace_feature {
            return Ok(ParsedSimpleSelector::Unsupported {
                span: block_span,
                features: vec![UnsupportedSelectorFeature::Namespace],
            });
        }

        skip_attribute_trivia(values, &mut index);

        if index >= values.len() {
            let selector = AttributeExistsSelector::new(block_span, name).map_err(|error| {
                SegmentParseError::Invalid {
                    span: Some(block_span),
                    reason: map_structure_error(error),
                }
            })?;
            return Ok(ParsedSimpleSelector::Subclass {
                span: block_span,
                selector: SubclassSelector::Attribute(AttributeSelector::Exists(selector)),
            });
        }

        let matcher = parse_attribute_matcher(values, &mut index)?;
        skip_attribute_trivia(values, &mut index);

        if index >= values.len() {
            return Err(SegmentParseError::Invalid {
                span: Some(block_span),
                reason: InvalidSelectorReason::MissingAttributeValue,
            });
        }

        let value = parse_attribute_value(self.input, values, &mut index)?;
        skip_attribute_trivia(values, &mut index);

        if index < values.len() {
            if attribute_case_modifier(self.input, values, &mut index) {
                skip_attribute_trivia(values, &mut index);
                if index == values.len() {
                    return Ok(ParsedSimpleSelector::Unsupported {
                        span: block_span,
                        features: vec![UnsupportedSelectorFeature::AttributeCaseModifier],
                    });
                }
            }

            return Err(SegmentParseError::Invalid {
                span: Some(name_span),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            });
        }

        let selector =
            AttributeMatchSelector::new(block_span, name, matcher, value).map_err(|error| {
                SegmentParseError::Invalid {
                    span: Some(block_span),
                    reason: map_structure_error(error),
                }
            })?;
        Ok(ParsedSimpleSelector::Subclass {
            span: block_span,
            selector: SubclassSelector::Attribute(AttributeSelector::Match(selector)),
        })
    }

    fn skip_trivia(&mut self) -> TriviaRun {
        let mut first_span = None;
        let mut saw_whitespace = false;

        while let Some(value) = self.current_value() {
            match value {
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Whitespace,
                    span,
                }) => {
                    saw_whitespace = true;
                    first_span.get_or_insert(*span);
                    self.index += 1;
                }
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Comment(_),
                    span,
                }) => {
                    first_span.get_or_insert(*span);
                    self.index += 1;
                }
                _ => break,
            }
        }

        TriviaRun {
            saw_whitespace,
            first_span,
        }
    }

    fn skip_comments(&mut self) {
        while matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comment(_),
                ..
            }))
        ) {
            self.index += 1;
        }
    }

    fn consume_explicit_combinator(&mut self) -> Option<ExplicitCombinator> {
        let token = self.current_token()?;

        let combinator = match token.kind {
            CssTokenKind::Delim('>') => ExplicitCombinator::Supported {
                combinator: Combinator::Child,
                span: token.span,
            },
            CssTokenKind::Delim('+') => ExplicitCombinator::Supported {
                combinator: Combinator::NextSibling,
                span: token.span,
            },
            CssTokenKind::Delim('~') => ExplicitCombinator::Supported {
                combinator: Combinator::SubsequentSibling,
                span: token.span,
            },
            CssTokenKind::Column => ExplicitCombinator::Unsupported {
                feature: UnsupportedSelectorFeature::ColumnCombinator,
                span: token.span,
            },
            _ => return None,
        };

        self.index += 1;
        Some(combinator)
    }

    fn current_value(&self) -> Option<&'a CssComponentValue> {
        self.values.get(self.index)
    }

    fn current_token(&self) -> Option<&'a CssToken> {
        match self.current_value()? {
            CssComponentValue::PreservedToken(token) => Some(token),
            CssComponentValue::SimpleBlock(_) | CssComponentValue::Function(_) => None,
        }
    }

    fn current_span(&self) -> Option<CssSpan> {
        self.current_value().map(component_value_span)
    }

    fn is_eof(&self) -> bool {
        self.index >= self.values.len()
    }

    fn current_is_whitespace(&self) -> bool {
        matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Whitespace,
                ..
            }))
        )
    }

    fn current_is_comma(&self) -> bool {
        matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comma,
                ..
            }))
        )
    }

    fn current_is_combinator(&self) -> bool {
        matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Delim('>' | '+' | '~') | CssTokenKind::Column,
                ..
            }))
        )
    }

    fn next_non_comment_is_namespace_delim(&self) -> bool {
        let mut index = self.index.saturating_add(1);
        while let Some(value) = self.values.get(index) {
            match value {
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Comment(_),
                    ..
                }) => index += 1,
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Delim('|'),
                    ..
                }) => return true,
                _ => return false,
            }
        }
        false
    }

    fn consume_namespace_sequence(&mut self) -> CssSpan {
        let start = self.current_span().expect("namespace sequence start");
        self.index += 1;
        self.skip_comments();

        if matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Delim('|'),
                ..
            }))
        ) {
            let delim_span = self.current_span().expect("namespace delimiter span");
            self.index += 1;
            self.skip_comments();

            if matches!(
                self.current_value(),
                Some(CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Ident(_) | CssTokenKind::Delim('*'),
                    ..
                }))
            ) {
                let end = self.current_span().expect("namespace local name span");
                self.index += 1;
                return span_from_bounds(start, end).expect("namespace sequence span");
            }

            return span_from_bounds(start, delim_span).expect("namespace prefix span");
        }

        start
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

fn parse_selector_segment(
    input: &CssInput,
    values: &[CssComponentValue],
    fallback_span: Option<CssSpan>,
) -> SegmentParseResult {
    SegmentParser {
        input,
        values,
        index: 0,
        unsupported: Vec::new(),
    }
    .parse(fallback_span)
}

fn top_level_comma_indices(values: &[CssComponentValue]) -> impl Iterator<Item = usize> + '_ {
    values.iter().enumerate().filter_map(|(index, value)| {
        matches!(
            value,
            CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comma,
                ..
            })
        )
        .then_some(index)
    })
}

fn is_trivia_component(value: &CssComponentValue) -> bool {
    matches!(
        value,
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::Whitespace | CssTokenKind::Comment(_),
            ..
        })
    )
}

fn component_value_span(value: &CssComponentValue) -> CssSpan {
    match value {
        CssComponentValue::PreservedToken(token) => token.span,
        CssComponentValue::SimpleBlock(block) => block.span,
        CssComponentValue::Function(function) => function.span,
    }
}

fn component_list_span(values: &[CssComponentValue]) -> Option<CssSpan> {
    let start = values.first().map(component_value_span)?;
    let end = values.last().map(component_value_span)?;
    span_from_bounds(start, end)
}

fn span_from_bounds(start: CssSpan, end: CssSpan) -> Option<CssSpan> {
    if start.input_id != end.input_id || end.end < start.start {
        return None;
    }

    CssSpan::new(start.input_id, start.start, end.end)
}

fn selector_ident_from_text(
    input: &CssInput,
    text: &CssTokenText,
) -> Result<SelectorIdent, SegmentParseError> {
    let value = text
        .resolve(input)
        .ok_or_else(|| SegmentParseError::Invalid {
            span: token_text_span(text),
            reason: InvalidSelectorReason::UnexpectedComponentValue,
        })?;

    SelectorIdent::new(value.into_owned(), token_text_span(text)).map_err(|error| {
        SegmentParseError::Invalid {
            span: token_text_span(text),
            reason: map_structure_error(error),
        }
    })
}

fn selector_string_from_text(
    input: &CssInput,
    text: &CssTokenText,
) -> Result<SelectorString, SegmentParseError> {
    let value = text
        .resolve(input)
        .ok_or_else(|| SegmentParseError::Invalid {
            span: token_text_span(text),
            reason: InvalidSelectorReason::UnexpectedComponentValue,
        })?;

    Ok(SelectorString::new(
        value.into_owned(),
        token_text_span(text),
    ))
}

fn token_text_span(text: &CssTokenText) -> Option<CssSpan> {
    match text {
        CssTokenText::Span(span) => Some(*span),
        CssTokenText::Owned(_) => None,
    }
}

fn map_structure_error(error: SelectorStructureError) -> InvalidSelectorReason {
    match error {
        SelectorStructureError::EmptySelectorList => InvalidSelectorReason::EmptySelectorList,
        SelectorStructureError::EmptyCompoundSelector => {
            InvalidSelectorReason::EmptyCompoundSelector
        }
        SelectorStructureError::EmptyIdentifier
        | SelectorStructureError::MixedInputIds
        | SelectorStructureError::NonMonotonicSpans
        | SelectorStructureError::PayloadSpanOutsideNode => {
            InvalidSelectorReason::UnexpectedComponentValue
        }
    }
}

fn function_name_is_forgiving_list(input: &CssInput, name: &CssTokenText) -> bool {
    matches!(name.resolve(input).as_deref(), Some("is") | Some("where"))
}

fn skip_attribute_trivia(values: &[CssComponentValue], index: &mut usize) {
    while let Some(value) = values.get(*index) {
        if is_trivia_component(value) {
            *index += 1;
        } else {
            break;
        }
    }
}

fn parse_attribute_name(
    input: &CssInput,
    values: &[CssComponentValue],
    index: &mut usize,
) -> Result<(SelectorIdent, CssSpan, bool), SegmentParseError> {
    let Some(value) = values.get(*index) else {
        return Err(SegmentParseError::Invalid {
            span: None,
            reason: InvalidSelectorReason::MissingAttributeName,
        });
    };

    match value {
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::Ident(text),
            span,
        }) => {
            let is_namespace = next_attribute_non_comment_is_namespace_delim(values, *index);
            let name = selector_ident_from_text(input, text)?;
            if is_namespace {
                *index = consume_attribute_namespace_sequence(values, *index);
                Ok((name, *span, true))
            } else {
                *index += 1;
                Ok((name, *span, false))
            }
        }
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::Delim('*') | CssTokenKind::Delim('|'),
            span,
        }) => {
            let placeholder =
                SelectorIdent::new("*", Some(*span)).expect("attribute namespace placeholder");
            *index = consume_attribute_namespace_sequence(values, *index);
            Ok((placeholder, *span, true))
        }
        _ => Err(SegmentParseError::Invalid {
            span: Some(component_value_span(value)),
            reason: InvalidSelectorReason::MissingAttributeName,
        }),
    }
}

fn next_attribute_non_comment_is_namespace_delim(
    values: &[CssComponentValue],
    index: usize,
) -> bool {
    let mut next = index.saturating_add(1);
    while let Some(value) = values.get(next) {
        match value {
            CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comment(_),
                ..
            }) => next += 1,
            CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Delim('|'),
                ..
            }) => return true,
            _ => return false,
        }
    }
    false
}

fn consume_attribute_namespace_sequence(values: &[CssComponentValue], mut index: usize) -> usize {
    index += 1;
    while let Some(value) = values.get(index) {
        match value {
            CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comment(_),
                ..
            }) => index += 1,
            CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Delim('|'),
                ..
            }) => {
                index += 1;
                while let Some(value) = values.get(index) {
                    match value {
                        CssComponentValue::PreservedToken(CssToken {
                            kind: CssTokenKind::Comment(_),
                            ..
                        }) => index += 1,
                        CssComponentValue::PreservedToken(CssToken {
                            kind: CssTokenKind::Ident(_) | CssTokenKind::Delim('*'),
                            ..
                        }) => return index + 1,
                        _ => return index,
                    }
                }
                return index;
            }
            _ => return index,
        }
    }

    index
}

fn parse_attribute_matcher(
    values: &[CssComponentValue],
    index: &mut usize,
) -> Result<AttributeMatcher, SegmentParseError> {
    let Some(value) = values.get(*index) else {
        return Err(SegmentParseError::Invalid {
            span: None,
            reason: InvalidSelectorReason::UnexpectedComponentValue,
        });
    };

    let matcher = match value {
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::Delim('='),
            ..
        }) => AttributeMatcher::Exact,
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::IncludeMatch,
            ..
        }) => AttributeMatcher::Includes,
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::DashMatch,
            ..
        }) => AttributeMatcher::DashMatch,
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::PrefixMatch,
            ..
        }) => AttributeMatcher::Prefix,
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::SuffixMatch,
            ..
        }) => AttributeMatcher::Suffix,
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::SubstringMatch,
            ..
        }) => AttributeMatcher::Substring,
        _ => {
            return Err(SegmentParseError::Invalid {
                span: Some(component_value_span(value)),
                reason: InvalidSelectorReason::UnexpectedComponentValue,
            });
        }
    };

    *index += 1;
    Ok(matcher)
}

fn parse_attribute_value(
    input: &CssInput,
    values: &[CssComponentValue],
    index: &mut usize,
) -> Result<AttributeValue, SegmentParseError> {
    let Some(value) = values.get(*index) else {
        return Err(SegmentParseError::Invalid {
            span: None,
            reason: InvalidSelectorReason::MissingAttributeValue,
        });
    };

    let value = match value {
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::Ident(text),
            ..
        }) => AttributeValue::ident(selector_ident_from_text(input, text)?),
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::String(text),
            ..
        }) => AttributeValue::string(selector_string_from_text(input, text)?),
        _ => {
            return Err(SegmentParseError::Invalid {
                span: Some(component_value_span(value)),
                reason: InvalidSelectorReason::MissingAttributeValue,
            });
        }
    };

    *index += 1;
    Ok(value)
}

fn attribute_case_modifier(
    input: &CssInput,
    values: &[CssComponentValue],
    index: &mut usize,
) -> bool {
    let Some(value) = values.get(*index) else {
        return false;
    };

    let CssComponentValue::PreservedToken(CssToken {
        kind: CssTokenKind::Ident(text),
        ..
    }) = value
    else {
        return false;
    };

    let is_modifier = matches!(
        text.resolve(input).as_deref(),
        Some("i") | Some("I") | Some("s") | Some("S")
    );

    if is_modifier {
        *index += 1;
    }

    is_modifier
}
