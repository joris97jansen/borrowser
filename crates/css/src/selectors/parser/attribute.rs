use super::convert::{map_structure_error, selector_ident_from_text, selector_string_from_text};
use super::segment::{ParsedSimpleSelector, SegmentParseError, SegmentParser};
use super::spans::component_value_span;
use super::trivia::is_trivia_component;
use super::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue, CssComponentValue, CssInput, CssSpan, CssToken, CssTokenKind,
    InvalidSelectorReason, SelectorIdent, SubclassSelector, UnsupportedSelectorFeature,
};

impl<'a> SegmentParser<'a> {
    pub(super) fn parse_attribute_selector(
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
                SelectorIdent::new("*", Some(*span)).map_err(|_| SegmentParseError::Invalid {
                    span: Some(*span),
                    reason: InvalidSelectorReason::InvariantViolation,
                })?;
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
