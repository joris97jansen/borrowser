use super::config::{
    CssParserFuzzConfig, CssParserFuzzSummary, CssParserFuzzTermination, CssSyntaxFuzzError,
};
use super::digest::{mix_bool, mix_u64, mix_usize};
use super::invariants::ensure_span_within;
use super::observed_digest::{mix_span, mix_token, mix_token_text, observe_diagnostics};
use crate::syntax::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclarationBlock, CssFunction, CssInput,
    CssQualifiedRule, CssRule, CssSimpleBlock, CssSpan, StylesheetParse,
};

pub(super) struct ParserObserver<'a> {
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
    pub(super) fn new(
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

    pub(super) fn finish(self) -> CssParserFuzzSummary {
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

    pub(super) fn observe_parse(
        &mut self,
        parse: &StylesheetParse,
    ) -> Result<(), CssSyntaxFuzzError> {
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
