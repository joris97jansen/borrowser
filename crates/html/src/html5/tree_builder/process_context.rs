use crate::html5::shared::ObservedInsertionMode;
use crate::html5::shared::Token;
use crate::html5::shared::{
    AtomTable, DocumentParseContext, ParseErrorCode, ParserContextSummary, ParserDiagnosticSink,
    ParserRecoveryAction, ParserResourceLimit, ParserTokenKind,
    TreeConstructionImplementationDiagnosticCode, TreeConstructionParseErrorCode,
};
use crate::html5::tree_builder::TreeBuilderError;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::names::ElementNamespace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeBuilderTokenSource {
    DirectTokenInjection,
    IntegratedTokenizer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum SelfClosingFlagEffect {
    NotPresent,
    Acknowledged,
    IgnoredWhileUnacknowledged,
    AlteredHtmlStackDisposition,
}

pub(crate) struct TreeConstructionDiagnosticSink<'a> {
    shared: ParserDiagnosticSink<'a>,
}

impl TreeConstructionDiagnosticSink<'_> {
    fn record_parse_error(
        &mut self,
        code: TreeConstructionParseErrorCode,
        recovery: Option<ParserRecoveryAction>,
        context: ParserContextSummary,
        description: Option<&'static str>,
    ) {
        self.shared.record_tree_parse_error(
            ParseErrorCode::TreeConstruction(code),
            recovery,
            context,
            description,
        );
    }

    fn record_implementation_diagnostic(
        &mut self,
        code: TreeConstructionImplementationDiagnosticCode,
        context: ParserContextSummary,
        description: Option<&'static str>,
    ) {
        self.shared
            .record_tree_implementation_diagnostic(code, context, description);
    }

    fn record_resource_limit(
        &mut self,
        limit: ParserResourceLimit,
        configured_limit: usize,
        context: ParserContextSummary,
        description: Option<&'static str>,
    ) {
        self.shared
            .record_tree_resource_limit(limit, configured_limit, context, description);
    }
}

/// Per-token tree-construction dependencies borrowed from the parser owner.
pub struct TreeBuilderProcessContext<'a> {
    atoms: &'a AtomTable,
    diagnostics: TreeConstructionDiagnosticSink<'a>,
    token_kind: ParserTokenKind,
    token_source: TreeBuilderTokenSource,
    self_closing_flag: SelfClosingFlagEffect,
}

impl<'a> TreeBuilderProcessContext<'a> {
    /// Construct a context for the public direct-token tree-builder API.
    pub fn new(parse_context: &'a mut DocumentParseContext) -> Self {
        Self::build(parse_context, TreeBuilderTokenSource::DirectTokenInjection)
    }

    pub(crate) fn for_integrated_parser(parse_context: &'a mut DocumentParseContext) -> Self {
        Self::build(parse_context, TreeBuilderTokenSource::IntegratedTokenizer)
    }

    fn build(
        parse_context: &'a mut DocumentParseContext,
        token_source: TreeBuilderTokenSource,
    ) -> Self {
        let DocumentParseContext {
            atoms,
            counters,
            error_policy,
            errors,
            observations,
        } = parse_context;
        Self {
            atoms,
            diagnostics: TreeConstructionDiagnosticSink {
                shared: ParserDiagnosticSink::new(counters, *error_policy, errors, observations),
            },
            token_kind: ParserTokenKind::Eof,
            token_source,
            self_closing_flag: SelfClosingFlagEffect::NotPresent,
        }
    }

    pub(in crate::html5::tree_builder) fn begin_token(&mut self, token: &Token) {
        self.token_kind = parser_token_kind(token);
        self.self_closing_flag = match token {
            Token::StartTag {
                self_closing: true, ..
            } => SelfClosingFlagEffect::IgnoredWhileUnacknowledged,
            _ => SelfClosingFlagEffect::NotPresent,
        };
    }

    pub fn atoms(&self) -> &'a AtomTable {
        self.atoms
    }

    pub(in crate::html5::tree_builder) fn is_integrated_token(&self) -> bool {
        self.token_source == TreeBuilderTokenSource::IntegratedTokenizer
    }

    pub(in crate::html5::tree_builder) fn record_parse_error(
        &mut self,
        code: TreeConstructionParseErrorCode,
        recovery: Option<ParserRecoveryAction>,
        insertion_mode: InsertionMode,
        adjusted_current_node_namespace: Option<ElementNamespace>,
        description: Option<&'static str>,
    ) {
        self.diagnostics.record_parse_error(
            code,
            recovery,
            self.parser_context(insertion_mode, adjusted_current_node_namespace),
            description,
        );
    }

    pub(in crate::html5::tree_builder) fn record_implementation_diagnostic(
        &mut self,
        code: TreeConstructionImplementationDiagnosticCode,
        insertion_mode: InsertionMode,
        adjusted_current_node_namespace: Option<ElementNamespace>,
        description: Option<&'static str>,
    ) {
        self.diagnostics.record_implementation_diagnostic(
            code,
            self.parser_context(insertion_mode, adjusted_current_node_namespace),
            description,
        );
    }

    pub(in crate::html5::tree_builder) fn record_resource_limit(
        &mut self,
        limit: ParserResourceLimit,
        configured_limit: usize,
        insertion_mode: InsertionMode,
        adjusted_current_node_namespace: Option<ElementNamespace>,
        description: Option<&'static str>,
    ) {
        self.diagnostics.record_resource_limit(
            limit,
            configured_limit,
            self.parser_context(insertion_mode, adjusted_current_node_namespace),
            description,
        );
    }

    pub(in crate::html5::tree_builder) fn acknowledge_self_closing_flag(
        &mut self,
    ) -> Result<(), TreeBuilderError> {
        self.transition_self_closing_flag(SelfClosingFlagEffect::Acknowledged)
    }

    pub(in crate::html5::tree_builder) fn mark_self_closing_flag_altered_html_stack_disposition(
        &mut self,
    ) -> Result<(), TreeBuilderError> {
        self.transition_self_closing_flag(SelfClosingFlagEffect::AlteredHtmlStackDisposition)
    }

    fn transition_self_closing_flag(
        &mut self,
        next: SelfClosingFlagEffect,
    ) -> Result<(), TreeBuilderError> {
        match (self.self_closing_flag, next) {
            (SelfClosingFlagEffect::IgnoredWhileUnacknowledged, next) => {
                self.self_closing_flag = next;
                Ok(())
            }
            (current, next) if current == next => Ok(()),
            _ => Err(crate::html5::shared::EngineInvariantError),
        }
    }

    pub(in crate::html5::tree_builder) fn finalize_self_closing_flag(
        &mut self,
        insertion_mode: InsertionMode,
        adjusted_current_node_namespace: Option<ElementNamespace>,
    ) {
        let recovery = match self.self_closing_flag {
            SelfClosingFlagEffect::NotPresent | SelfClosingFlagEffect::Acknowledged => return,
            SelfClosingFlagEffect::IgnoredWhileUnacknowledged => {
                Some(ParserRecoveryAction::IgnoreSelfClosingFlag)
            }
            SelfClosingFlagEffect::AlteredHtmlStackDisposition => None,
        };
        self.record_parse_error(
            TreeConstructionParseErrorCode::UnacknowledgedSelfClosingFlag,
            recovery,
            insertion_mode,
            adjusted_current_node_namespace,
            Some("non-void-html-element-start-tag-with-trailing-solidus"),
        );
    }

    fn parser_context(
        &self,
        insertion_mode: InsertionMode,
        adjusted_current_node_namespace: Option<ElementNamespace>,
    ) -> ParserContextSummary {
        ParserContextSummary {
            token_kind: Some(self.token_kind),
            insertion_mode: Some(observed_insertion_mode(insertion_mode)),
            adjusted_current_node_namespace,
        }
    }
}

fn parser_token_kind(token: &Token) -> ParserTokenKind {
    match token {
        Token::Doctype { .. } => ParserTokenKind::Doctype,
        Token::StartTag { .. } => ParserTokenKind::StartTag,
        Token::EndTag { .. } => ParserTokenKind::EndTag,
        Token::Text { .. } => ParserTokenKind::Character,
        Token::Comment { .. } => ParserTokenKind::Comment,
        Token::ProcessingInstruction(_) => ParserTokenKind::ProcessingInstruction,
        Token::Eof => ParserTokenKind::Eof,
    }
}

fn observed_insertion_mode(mode: InsertionMode) -> ObservedInsertionMode {
    match mode {
        InsertionMode::Initial => ObservedInsertionMode::Initial,
        InsertionMode::BeforeHtml => ObservedInsertionMode::BeforeHtml,
        InsertionMode::BeforeHead => ObservedInsertionMode::BeforeHead,
        InsertionMode::InHead => ObservedInsertionMode::InHead,
        InsertionMode::AfterHead => ObservedInsertionMode::AfterHead,
        InsertionMode::InBody => ObservedInsertionMode::InBody,
        InsertionMode::AfterBody => ObservedInsertionMode::AfterBody,
        InsertionMode::AfterAfterBody => ObservedInsertionMode::AfterAfterBody,
        InsertionMode::InTable => ObservedInsertionMode::InTable,
        InsertionMode::InTableText => ObservedInsertionMode::InTableText,
        InsertionMode::InCaption => ObservedInsertionMode::InCaption,
        InsertionMode::InColumnGroup => ObservedInsertionMode::InColumnGroup,
        InsertionMode::InTableBody => ObservedInsertionMode::InTableBody,
        InsertionMode::InRow => ObservedInsertionMode::InRow,
        InsertionMode::InCell => ObservedInsertionMode::InCell,
        InsertionMode::InTemplate => ObservedInsertionMode::InTemplate,
        InsertionMode::Text => ObservedInsertionMode::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::{SelfClosingFlagEffect, TreeBuilderProcessContext};
    use crate::html5::shared::{DocumentParseContext, TextSpan, TextValue, Token};
    use crate::html5::tokenizer::{TextResolveError, TextResolver};
    use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

    struct EmptyResolver;

    impl TextResolver for EmptyResolver {
        fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
            Err(TextResolveError::InvalidSpan { span })
        }
    }

    #[test]
    fn contradictory_self_closing_transitions_are_fallible_and_atomic() {
        let mut context = DocumentParseContext::with_tree_observations_for_test();
        let name = context
            .atoms
            .intern_ascii_folded("x-test")
            .expect("test atom");
        let token_without_flag = Token::StartTag {
            name,
            attrs: Vec::new(),
            self_closing: false,
        };
        let token_with_flag = Token::StartTag {
            name,
            attrs: Vec::new(),
            self_closing: true,
        };

        {
            let mut process = TreeBuilderProcessContext::new(&mut context);
            process.begin_token(&token_without_flag);
            assert!(process.acknowledge_self_closing_flag().is_err());
            assert_eq!(process.self_closing_flag, SelfClosingFlagEffect::NotPresent);
            assert!(
                process
                    .mark_self_closing_flag_altered_html_stack_disposition()
                    .is_err()
            );
            assert_eq!(process.self_closing_flag, SelfClosingFlagEffect::NotPresent);
        }
        assert_eq!(context.counters.parse_errors, 0);
        let capture = context.take_observations().expect("explicit observation");
        assert!(capture.parse_errors.items.is_empty());
        assert!(capture.implementation_diagnostics.items.is_empty());

        let mut acknowledged_context = DocumentParseContext::with_tree_observations_for_test();
        {
            let mut process = TreeBuilderProcessContext::new(&mut acknowledged_context);
            process.begin_token(&token_with_flag);
            process
                .acknowledge_self_closing_flag()
                .expect("first acknowledgement");
            process
                .acknowledge_self_closing_flag()
                .expect("identical acknowledgement is idempotent");
            assert!(
                process
                    .mark_self_closing_flag_altered_html_stack_disposition()
                    .is_err()
            );
            assert_eq!(
                process.self_closing_flag,
                SelfClosingFlagEffect::Acknowledged
            );
        }
        assert_eq!(acknowledged_context.counters.parse_errors, 0);
        let acknowledged_capture = acknowledged_context
            .take_observations()
            .expect("acknowledged capture");
        assert!(acknowledged_capture.parse_errors.items.is_empty());
        assert!(
            acknowledged_capture
                .implementation_diagnostics
                .items
                .is_empty()
        );

        let mut altered_context = DocumentParseContext::with_tree_observations_for_test();
        {
            let mut process = TreeBuilderProcessContext::new(&mut altered_context);
            process.begin_token(&token_with_flag);
            process
                .mark_self_closing_flag_altered_html_stack_disposition()
                .expect("first altered-stack disposition");
            process
                .mark_self_closing_flag_altered_html_stack_disposition()
                .expect("identical altered-stack transition is idempotent");
            assert!(process.acknowledge_self_closing_flag().is_err());
            assert_eq!(
                process.self_closing_flag,
                SelfClosingFlagEffect::AlteredHtmlStackDisposition
            );
        }
        assert_eq!(altered_context.counters.parse_errors, 0);
        let altered_capture = altered_context
            .take_observations()
            .expect("altered capture");
        assert!(altered_capture.parse_errors.items.is_empty());
        assert!(altered_capture.implementation_diagnostics.items.is_empty());
    }

    #[test]
    fn direct_process_context_constructor_preserves_observation_configuration() {
        let resolver = EmptyResolver;
        let mut ordinary = DocumentParseContext::new();
        assert!(!ordinary.observation_enabled());
        let mut builder =
            Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ordinary).expect("builder");
        let token = Token::Text {
            text: TextValue::Owned("x".to_string()),
        };
        let _ = builder
            .process(
                &token,
                &mut TreeBuilderProcessContext::new(&mut ordinary),
                &resolver,
            )
            .expect("malformed input remains recoverable");
        assert!(!ordinary.observation_enabled());
        assert_eq!(
            ordinary.counters.parse_errors, 1,
            "genuine tree errors count without a recorder"
        );
    }

    #[test]
    fn explicitly_observed_and_unobserved_direct_runs_are_output_identical() {
        fn run(
            observed: bool,
        ) -> (
            crate::html5::tree_builder::DomInvariantState,
            Vec<crate::DomPatch>,
            crate::html5::tree_builder::TreeBuilderStepResult,
            crate::html5::shared::Counters,
            bool,
        ) {
            let resolver = EmptyResolver;
            let mut context = if observed {
                DocumentParseContext::with_tree_observations_for_test()
            } else {
                DocumentParseContext::new()
            };
            let mut builder =
                Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut context).expect("builder");
            let div = context.atoms.intern_ascii_folded("div").expect("div atom");
            let step = builder
                .process(
                    &Token::StartTag {
                        name: div,
                        attrs: Vec::new(),
                        self_closing: true,
                    },
                    &mut TreeBuilderProcessContext::new(&mut context),
                    &resolver,
                )
                .expect("direct token");
            (
                builder.dom_invariant_state(),
                builder.drain_patches(),
                step,
                context.counters.clone(),
                context.observation_enabled(),
            )
        }

        let ordinary = run(false);
        let observed = run(true);
        assert_eq!(ordinary.0, observed.0);
        assert_eq!(ordinary.1, observed.1);
        assert_eq!(ordinary.2, observed.2);
        assert_eq!(ordinary.3, observed.3);
        assert!(!ordinary.4);
        assert!(observed.4);
    }
}
