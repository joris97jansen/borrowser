use super::config::{
    CssParserFuzzConfig, CssParserFuzzSummary, CssParserFuzzTermination, CssSyntaxFuzzError,
    CssTokenizerFuzzConfig, CssTokenizerFuzzSummary, CssTokenizerFuzzTermination,
};
use super::digest::{mix_bool, mix_str, mix_u64, mix_usize};
use crate::syntax::parser::validate_token_stream_invariants;
use crate::syntax::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclarationBlock, CssFunction, CssInput,
    CssParseOrigin, CssQualifiedRule, CssRule, CssSimpleBlock, CssSpan, CssStylesheet, CssToken,
    CssTokenKind, CssTokenText, DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats,
    StylesheetParse, SyntaxDiagnostic, SyntaxLimits, parse_stylesheet_with_options,
    tokenize_str_with_options,
};

pub fn run_seeded_tokenizer_fuzz_case(
    bytes: &[u8],
    config: CssTokenizerFuzzConfig,
) -> Result<CssTokenizerFuzzSummary, CssSyntaxFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(CssTokenizerFuzzSummary {
            seed: config.seed,
            termination: CssTokenizerFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            tokens_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let decoded = String::from_utf8_lossy(bytes).into_owned();
    if decoded.len() > config.max_decoded_bytes {
        return Ok(CssTokenizerFuzzSummary {
            seed: config.seed,
            termination: CssTokenizerFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            tokens_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let options = tokenizer_fuzz_options(&config);
    let tokenization = tokenize_str_with_options(&decoded, &options);
    validate_css_token_stream(&tokenization.input, &tokenization.tokens, &options)?;

    let mut digest = mix_usize(0, tokenization.stats.input_bytes);
    digest = mix_usize(digest, tokenization.stats.tokens_emitted);
    digest = mix_usize(digest, tokenization.stats.diagnostics_emitted);
    digest = mix_bool(digest, tokenization.stats.hit_limit);

    let mut tokens_observed = 0usize;
    for token in &tokenization.tokens {
        if tokens_observed >= config.max_tokens_observed {
            return Ok(CssTokenizerFuzzSummary {
                seed: config.seed,
                termination: CssTokenizerFuzzTermination::RejectedMaxTokensObserved,
                input_bytes: bytes.len(),
                decoded_bytes: decoded.len(),
                tokens_observed,
                diagnostics_observed: 0,
                hit_limit: tokenization.stats.hit_limit,
                digest,
            });
        }
        digest = mix_token(digest, &tokenization.input, token, "tokenizer token")?;
        tokens_observed += 1;
    }

    let (diagnostics_observed, digest, termination) = observe_diagnostics(
        &tokenization.input,
        &tokenization.diagnostics,
        config.max_diagnostics_observed,
        digest,
        CssTokenizerFuzzTermination::Completed,
        CssTokenizerFuzzTermination::RejectedMaxDiagnosticsObserved,
        "tokenizer diagnostics",
    )?;

    Ok(CssTokenizerFuzzSummary {
        seed: config.seed,
        termination,
        input_bytes: bytes.len(),
        decoded_bytes: decoded.len(),
        tokens_observed,
        diagnostics_observed,
        hit_limit: tokenization.stats.hit_limit,
        digest,
    })
}

pub fn run_seeded_parser_fuzz_case(
    bytes: &[u8],
    config: CssParserFuzzConfig,
) -> Result<CssParserFuzzSummary, CssSyntaxFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(CssParserFuzzSummary {
            seed: config.seed,
            termination: CssParserFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            rules_observed: 0,
            declarations_observed: 0,
            component_values_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let decoded = String::from_utf8_lossy(bytes).into_owned();
    if decoded.len() > config.max_decoded_bytes {
        return Ok(CssParserFuzzSummary {
            seed: config.seed,
            termination: CssParserFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            rules_observed: 0,
            declarations_observed: 0,
            component_values_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let options = parser_fuzz_options(&config);
    let parse = parse_stylesheet_with_options(&decoded, &options);
    ensure_parse_stats_consistent(&parse, "stylesheet parse")?;

    let mut observer =
        ParserObserver::new(&config, bytes.len(), decoded.len(), parse.stats.hit_limit);
    observer.observe_parse(&parse)?;
    Ok(observer.finish())
}

fn tokenizer_fuzz_options(config: &CssTokenizerFuzzConfig) -> ParseOptions {
    ParseOptions {
        origin: CssParseOrigin::Stylesheet,
        limits: SyntaxLimits {
            max_stylesheet_input_bytes: config.max_decoded_bytes,
            max_lexical_tokens: config.max_tokens_observed.saturating_add(1).max(1),
            max_diagnostics: config.max_diagnostics_observed.max(1),
            ..SyntaxLimits::default()
        },
        ..ParseOptions::stylesheet()
    }
}

fn parser_fuzz_options(config: &CssParserFuzzConfig) -> ParseOptions {
    let mut limits = config.syntax_limits.clone();
    limits.max_stylesheet_input_bytes = limits
        .max_stylesheet_input_bytes
        .min(config.max_decoded_bytes);
    limits.max_diagnostics = limits
        .max_diagnostics
        .min(config.max_diagnostics_observed.max(1));
    ParseOptions {
        origin: CssParseOrigin::Stylesheet,
        limits,
        ..ParseOptions::stylesheet()
    }
}

fn validate_css_token_stream(
    input: &CssInput,
    tokens: &[CssToken],
    options: &ParseOptions,
) -> Result<(), CssSyntaxFuzzError> {
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats::default();
    if validate_token_stream_invariants(options, input, tokens, 0, &mut diagnostics, &mut stats) {
        return Ok(());
    }

    let detail = diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| "token stream validation failed without a diagnostic".to_string());
    Err(CssSyntaxFuzzError::TokenStreamInvariantViolation { detail })
}

fn ensure_parse_stats_consistent(
    parse: &StylesheetParse,
    phase: &'static str,
) -> Result<(), CssSyntaxFuzzError> {
    if parse.stats.input_bytes != parse.input.len_bytes() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.input_bytes={} but input.len_bytes()={}",
                parse.stats.input_bytes,
                parse.input.len_bytes()
            ),
        });
    }
    if parse.stats.rules_emitted != parse.stylesheet.rules.len() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.rules_emitted={} but stylesheet.rules.len()={}",
                parse.stats.rules_emitted,
                parse.stylesheet.rules.len()
            ),
        });
    }
    let declaration_count = count_stylesheet_declarations(&parse.stylesheet);
    if parse.stats.declarations_emitted != declaration_count {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.declarations_emitted={} but counted declarations={declaration_count}",
                parse.stats.declarations_emitted
            ),
        });
    }
    if parse.stats.diagnostics_emitted < parse.diagnostics.len() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.diagnostics_emitted={} but diagnostics.len()={}",
                parse.stats.diagnostics_emitted,
                parse.diagnostics.len()
            ),
        });
    }
    Ok(())
}

fn count_stylesheet_declarations(stylesheet: &CssStylesheet) -> usize {
    stylesheet
        .rules
        .iter()
        .map(|rule| match rule {
            CssRule::Qualified(rule) => rule.block.declarations.len(),
            CssRule::At(_) => 0,
        })
        .sum()
}

fn mix_diagnostic(mut digest: u64, diagnostic: &SyntaxDiagnostic) -> u64 {
    digest = mix_u64(
        digest,
        match diagnostic.severity {
            DiagnosticSeverity::Warning => 1,
            DiagnosticSeverity::Error => 2,
        },
    );
    digest = mix_u64(
        digest,
        match diagnostic.kind {
            DiagnosticKind::UnexpectedEof => 1,
            DiagnosticKind::UnexpectedToken => 2,
            DiagnosticKind::InvariantViolation => 3,
            DiagnosticKind::EmptySelectorList => 4,
            DiagnosticKind::InvalidSelector => 5,
            DiagnosticKind::UnsupportedSelector => 6,
            DiagnosticKind::InvalidDeclaration => 7,
            DiagnosticKind::UnterminatedComment => 8,
            DiagnosticKind::UnterminatedString => 9,
            DiagnosticKind::BadUrl => 10,
            DiagnosticKind::LimitExceeded => 11,
        },
    );
    mix_usize(digest, diagnostic.byte_offset)
}

fn mix_token(
    mut digest: u64,
    input: &CssInput,
    token: &CssToken,
    phase: &'static str,
) -> Result<u64, CssSyntaxFuzzError> {
    ensure_span_in_input(input, token.span, phase)?;
    digest = mix_span(digest, token.span);
    digest = mix_u64(
        digest,
        match &token.kind {
            CssTokenKind::Whitespace => 1,
            CssTokenKind::Comment(_) => 2,
            CssTokenKind::Ident(_) => 3,
            CssTokenKind::Function(_) => 4,
            CssTokenKind::AtKeyword(_) => 5,
            CssTokenKind::Hash { .. } => 6,
            CssTokenKind::String(_) => 7,
            CssTokenKind::BadString => 8,
            CssTokenKind::Url(_) => 9,
            CssTokenKind::BadUrl => 10,
            CssTokenKind::Delim(_) => 11,
            CssTokenKind::Number(_) => 12,
            CssTokenKind::Percentage(_) => 13,
            CssTokenKind::Dimension(_) => 14,
            CssTokenKind::UnicodeRange(_) => 15,
            CssTokenKind::Colon => 16,
            CssTokenKind::Semicolon => 17,
            CssTokenKind::Comma => 18,
            CssTokenKind::LeftSquareBracket => 19,
            CssTokenKind::RightSquareBracket => 20,
            CssTokenKind::LeftParenthesis => 21,
            CssTokenKind::RightParenthesis => 22,
            CssTokenKind::LeftCurlyBracket => 23,
            CssTokenKind::RightCurlyBracket => 24,
            CssTokenKind::IncludeMatch => 25,
            CssTokenKind::DashMatch => 26,
            CssTokenKind::PrefixMatch => 27,
            CssTokenKind::SuffixMatch => 28,
            CssTokenKind::SubstringMatch => 29,
            CssTokenKind::Column => 30,
            CssTokenKind::Cdo => 31,
            CssTokenKind::Cdc => 32,
            CssTokenKind::Eof => 33,
        },
    );

    match &token.kind {
        CssTokenKind::Comment(text)
        | CssTokenKind::Ident(text)
        | CssTokenKind::Function(text)
        | CssTokenKind::AtKeyword(text)
        | CssTokenKind::String(text)
        | CssTokenKind::Url(text) => {
            digest = mix_token_text(digest, input, text, phase)?;
        }
        CssTokenKind::Hash { value, kind } => {
            digest = mix_u64(
                digest,
                match kind {
                    crate::syntax::CssHashKind::Id => 1,
                    crate::syntax::CssHashKind::Unrestricted => 2,
                },
            );
            digest = mix_token_text(digest, input, value, phase)?;
        }
        CssTokenKind::Delim(value) => {
            digest = mix_u64(digest, u64::from(*value));
        }
        CssTokenKind::Number(number) | CssTokenKind::Percentage(number) => {
            digest = mix_u64(
                digest,
                match number.kind {
                    crate::syntax::CssNumericKind::Integer => 1,
                    crate::syntax::CssNumericKind::Number => 2,
                },
            );
            digest = mix_token_text(digest, input, &number.repr, phase)?;
        }
        CssTokenKind::Dimension(dimension) => {
            digest = mix_u64(
                digest,
                match dimension.number.kind {
                    crate::syntax::CssNumericKind::Integer => 1,
                    crate::syntax::CssNumericKind::Number => 2,
                },
            );
            digest = mix_token_text(digest, input, &dimension.number.repr, phase)?;
            digest = mix_token_text(digest, input, &dimension.unit, phase)?;
        }
        CssTokenKind::UnicodeRange(range) => {
            digest = mix_u64(digest, u64::from(range.start()));
            digest = mix_u64(digest, u64::from(range.end()));
        }
        CssTokenKind::Whitespace
        | CssTokenKind::BadString
        | CssTokenKind::BadUrl
        | CssTokenKind::Colon
        | CssTokenKind::Semicolon
        | CssTokenKind::Comma
        | CssTokenKind::LeftSquareBracket
        | CssTokenKind::RightSquareBracket
        | CssTokenKind::LeftParenthesis
        | CssTokenKind::RightParenthesis
        | CssTokenKind::LeftCurlyBracket
        | CssTokenKind::RightCurlyBracket
        | CssTokenKind::IncludeMatch
        | CssTokenKind::DashMatch
        | CssTokenKind::PrefixMatch
        | CssTokenKind::SuffixMatch
        | CssTokenKind::SubstringMatch
        | CssTokenKind::Column
        | CssTokenKind::Cdo
        | CssTokenKind::Cdc
        | CssTokenKind::Eof => {}
    }

    Ok(digest)
}

fn mix_token_text(
    digest: u64,
    input: &CssInput,
    text: &CssTokenText,
    phase: &'static str,
) -> Result<u64, CssSyntaxFuzzError> {
    match text.resolve(input) {
        Some(text) => Ok(mix_str(digest, text.as_ref())),
        None => Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: "span-backed token text failed to resolve against its owning input".to_string(),
        }),
    }
}

fn mix_span(mut digest: u64, span: CssSpan) -> u64 {
    digest = mix_usize(digest, span.start);
    mix_usize(digest, span.end)
}

fn ensure_span_in_input(
    input: &CssInput,
    span: CssSpan,
    phase: &'static str,
) -> Result<(), CssSyntaxFuzzError> {
    if span.input_id != input.id() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: "span belongs to a different input".to_string(),
        });
    }
    if input.slice(span).is_none() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "span @{}..{} is out of bounds or not on UTF-8 boundaries",
                span.start, span.end
            ),
        });
    }
    Ok(())
}

fn ensure_span_within(
    input: &CssInput,
    outer: CssSpan,
    inner: CssSpan,
    phase: &'static str,
) -> Result<(), CssSyntaxFuzzError> {
    ensure_span_in_input(input, inner, phase)?;
    if outer.input_id != inner.input_id || inner.start < outer.start || inner.end > outer.end {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "child span @{}..{} escapes parent span @{}..{}",
                inner.start, inner.end, outer.start, outer.end
            ),
        });
    }
    Ok(())
}

fn observe_diagnostics<T: Copy>(
    input: &CssInput,
    diagnostics: &[SyntaxDiagnostic],
    max_observed: usize,
    mut digest: u64,
    completed: T,
    rejected: T,
    phase: &'static str,
) -> Result<(usize, u64, T), CssSyntaxFuzzError> {
    let mut observed = 0usize;
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if observed >= max_observed {
            return Ok((observed, digest, rejected));
        }
        if diagnostic.byte_offset > input.len_bytes() {
            return Err(CssSyntaxFuzzError::InvalidDiagnosticOffset {
                phase,
                diagnostic_index: index,
                byte_offset: diagnostic.byte_offset,
                input_bytes: input.len_bytes(),
            });
        }
        digest = mix_diagnostic(digest, diagnostic);
        observed += 1;
    }
    Ok((observed, digest, completed))
}

struct ParserObserver<'a> {
    config: &'a CssParserFuzzConfig,
    digest: u64,
    rules_observed: usize,
    declarations_observed: usize,
    component_values_observed: usize,
    diagnostics_observed: usize,
    hit_limit: bool,
    input_bytes: usize,
    decoded_bytes: usize,
    termination: CssParserFuzzTermination,
}

impl<'a> ParserObserver<'a> {
    fn new(
        config: &'a CssParserFuzzConfig,
        input_bytes: usize,
        decoded_bytes: usize,
        hit_limit: bool,
    ) -> Self {
        let mut digest = mix_usize(0, input_bytes);
        digest = mix_usize(digest, decoded_bytes);
        digest = mix_bool(digest, hit_limit);
        digest = mix_usize(digest, config.syntax_limits.max_stylesheet_input_bytes);
        digest = mix_usize(digest, config.syntax_limits.max_lexical_tokens);
        digest = mix_usize(digest, config.syntax_limits.max_rules);
        digest = mix_usize(
            digest,
            config.syntax_limits.max_component_values_per_container,
        );
        digest = mix_usize(digest, config.syntax_limits.max_component_nesting_depth);
        digest = mix_usize(digest, config.syntax_limits.max_diagnostics);
        Self {
            config,
            digest,
            rules_observed: 0,
            declarations_observed: 0,
            component_values_observed: 0,
            diagnostics_observed: 0,
            hit_limit,
            input_bytes,
            decoded_bytes,
            termination: CssParserFuzzTermination::Completed,
        }
    }

    fn finish(self) -> CssParserFuzzSummary {
        CssParserFuzzSummary {
            seed: self.config.seed,
            termination: self.termination,
            input_bytes: self.input_bytes,
            decoded_bytes: self.decoded_bytes,
            rules_observed: self.rules_observed,
            declarations_observed: self.declarations_observed,
            component_values_observed: self.component_values_observed,
            diagnostics_observed: self.diagnostics_observed,
            hit_limit: self.hit_limit,
            digest: self.digest,
        }
    }

    fn observe_parse(&mut self, parse: &StylesheetParse) -> Result<(), CssSyntaxFuzzError> {
        let full_span = parse
            .input
            .span(0, parse.input.len_bytes())
            .unwrap_or_else(|| parse.input.zero_span());

        self.digest = mix_usize(self.digest, parse.stats.input_bytes);
        self.digest = mix_usize(self.digest, parse.stats.rules_emitted);
        self.digest = mix_usize(self.digest, parse.stats.declarations_emitted);
        self.digest = mix_usize(self.digest, parse.stats.diagnostics_emitted);
        self.digest = mix_bool(self.digest, parse.stats.hit_limit);

        let mut previous_rule_end = 0usize;
        for rule in &parse.stylesheet.rules {
            if self.rules_observed >= self.config.max_rules_observed {
                self.termination = CssParserFuzzTermination::RejectedMaxRulesObserved;
                return Ok(());
            }
            let span = match rule {
                CssRule::Qualified(rule) => rule.span,
                CssRule::At(rule) => rule.span,
            };
            ensure_span_within(&parse.input, full_span, span, "stylesheet rule")?;
            if span.start < previous_rule_end {
                return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
                    phase: "stylesheet rule order",
                    detail: format!(
                        "rule span @{}..{} starts before previous rule ended at {}",
                        span.start, span.end, previous_rule_end
                    ),
                });
            }
            self.rules_observed += 1;
            self.digest = mix_span(self.digest, span);
            match rule {
                CssRule::Qualified(rule) => self.observe_qualified_rule(&parse.input, rule)?,
                CssRule::At(rule) => self.observe_at_rule(&parse.input, rule)?,
            }
            previous_rule_end = span.end;
        }

        let (diagnostics_observed, digest, termination) = observe_diagnostics(
            &parse.input,
            &parse.diagnostics,
            self.config.max_diagnostics_observed,
            self.digest,
            CssParserFuzzTermination::Completed,
            CssParserFuzzTermination::RejectedMaxDiagnosticsObserved,
            "stylesheet diagnostics",
        )?;
        self.diagnostics_observed = diagnostics_observed;
        self.digest = digest;
        if !matches!(self.termination, CssParserFuzzTermination::Completed) {
            return Ok(());
        }
        self.termination = termination;
        Ok(())
    }

    fn observe_qualified_rule(
        &mut self,
        input: &CssInput,
        rule: &CssQualifiedRule,
    ) -> Result<(), CssSyntaxFuzzError> {
        self.digest = mix_u64(self.digest, 1);
        self.observe_component_list(input, rule.span, &rule.prelude, "qualified prelude")?;
        self.observe_declaration_block(input, rule.span, &rule.block)?;
        Ok(())
    }

    fn observe_at_rule(
        &mut self,
        input: &CssInput,
        rule: &CssAtRule,
    ) -> Result<(), CssSyntaxFuzzError> {
        self.digest = mix_u64(self.digest, 2);
        self.digest = mix_token_text(self.digest, input, &rule.name, "at-rule name")?;
        self.observe_component_list(input, rule.span, &rule.prelude, "at-rule prelude")?;
        if let Some(block) = &rule.block {
            self.observe_simple_block(input, rule.span, block, "at-rule block")?;
        }
        Ok(())
    }

    fn observe_declaration_block(
        &mut self,
        input: &CssInput,
        parent_span: CssSpan,
        block: &CssDeclarationBlock,
    ) -> Result<(), CssSyntaxFuzzError> {
        ensure_span_within(input, parent_span, block.span, "declaration block")?;
        self.digest = mix_span(self.digest, block.span);

        let mut previous_declaration_end = block.span.start;
        for declaration in &block.declarations {
            if self.declarations_observed >= self.config.max_declarations_observed {
                self.termination = CssParserFuzzTermination::RejectedMaxDeclarationsObserved;
                return Ok(());
            }
            ensure_span_within(input, block.span, declaration.span, "declaration")?;
            if declaration.span.start < previous_declaration_end {
                return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
                    phase: "declaration order",
                    detail: format!(
                        "declaration span @{}..{} starts before previous declaration ended at {}",
                        declaration.span.start, declaration.span.end, previous_declaration_end
                    ),
                });
            }
            self.declarations_observed += 1;
            self.digest = mix_span(self.digest, declaration.span);
            self.digest =
                mix_token_text(self.digest, input, &declaration.name, "declaration name")?;
            ensure_span_within(
                input,
                declaration.span,
                declaration.value_span,
                "declaration value span",
            )?;
            self.digest = mix_span(self.digest, declaration.value_span);
            self.observe_component_list(
                input,
                declaration.value_span,
                &declaration.value,
                "declaration value",
            )?;
            if !matches!(self.termination, CssParserFuzzTermination::Completed) {
                return Ok(());
            }
            previous_declaration_end = declaration.span.end;
        }

        Ok(())
    }

    fn observe_component_list(
        &mut self,
        input: &CssInput,
        parent_span: CssSpan,
        values: &[CssComponentValue],
        phase: &'static str,
    ) -> Result<(), CssSyntaxFuzzError> {
        let mut previous_end = parent_span.start;
        for value in values {
            if self.component_values_observed >= self.config.max_component_values_observed {
                self.termination = CssParserFuzzTermination::RejectedMaxComponentValuesObserved;
                return Ok(());
            }
            let span = component_value_span(value);
            ensure_span_within(input, parent_span, span, phase)?;
            if span.start < previous_end {
                return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
                    phase,
                    detail: format!(
                        "component span @{}..{} starts before previous component ended at {}",
                        span.start, span.end, previous_end
                    ),
                });
            }
            self.component_values_observed += 1;
            self.digest = mix_span(self.digest, span);
            match value {
                CssComponentValue::PreservedToken(token) => {
                    self.digest = mix_token(self.digest, input, token, phase)?;
                }
                CssComponentValue::SimpleBlock(block) => {
                    self.observe_simple_block(input, parent_span, block, phase)?;
                }
                CssComponentValue::Function(function) => {
                    self.observe_function(input, parent_span, function, phase)?;
                }
            }
            if !matches!(self.termination, CssParserFuzzTermination::Completed) {
                return Ok(());
            }
            previous_end = span.end;
        }
        Ok(())
    }

    fn observe_simple_block(
        &mut self,
        input: &CssInput,
        parent_span: CssSpan,
        block: &CssSimpleBlock,
        phase: &'static str,
    ) -> Result<(), CssSyntaxFuzzError> {
        ensure_span_within(input, parent_span, block.span, phase)?;
        self.digest = mix_u64(
            self.digest,
            match block.kind {
                CssBlockKind::Curly => 1,
                CssBlockKind::Square => 2,
                CssBlockKind::Parenthesis => 3,
            },
        );
        self.observe_component_list(input, block.span, &block.value, phase)
    }

    fn observe_function(
        &mut self,
        input: &CssInput,
        parent_span: CssSpan,
        function: &CssFunction,
        phase: &'static str,
    ) -> Result<(), CssSyntaxFuzzError> {
        ensure_span_within(input, parent_span, function.span, phase)?;
        self.digest = mix_token_text(self.digest, input, &function.name, "function name")?;
        self.observe_component_list(input, function.span, &function.value, phase)
    }
}

fn component_value_span(value: &CssComponentValue) -> CssSpan {
    match value {
        CssComponentValue::PreservedToken(token) => token.span,
        CssComponentValue::SimpleBlock(block) => block.span,
        CssComponentValue::Function(function) => function.span,
    }
}
