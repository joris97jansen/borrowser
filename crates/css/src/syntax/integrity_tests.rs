//! Private one-shot corruption of internal invariants, never authored CSS errors.
use std::cell::Cell;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    TokenizerTextSpan,
    ParserMissingEof,
    ModelSelector,
    ModelAtRuleName,
}
thread_local! { static FAULT: Cell<Option<Fault>> = const { Cell::new(None) }; }
pub(crate) fn take(fault: Fault) -> bool {
    FAULT.with(|value| {
        if value.get() == Some(fault) {
            value.set(None);
            true
        } else {
            false
        }
    })
}
fn with_fault<T>(fault: Fault, action: impl FnOnce() -> T) -> T {
    struct Restore(Option<Fault>);
    impl Drop for Restore {
        fn drop(&mut self) {
            FAULT.with(|value| value.set(self.0));
        }
    }
    let previous = FAULT.with(|value| value.replace(Some(fault)));
    let _restore = Restore(previous);
    let result = action();
    assert_eq!(
        FAULT.with(Cell::get),
        None,
        "fault must reach its intended producer"
    );
    result
}
fn modes() -> [super::ParseOptions; 3] {
    let ordinary = super::ParseOptions::stylesheet();
    let mut disabled = ordinary.clone();
    disabled.collect_diagnostics = false;
    let mut zero = ordinary.clone();
    zero.limits.max_diagnostics = 0;
    [ordinary, disabled, zero]
}
#[test]
fn aj8_integrity_actual_tokenizer_producer_survives_suppression_and_merge() {
    for options in modes() {
        let tokens = with_fault(Fault::TokenizerTextSpan, || {
            super::tokenize_str_with_options("p{color:red}", &options)
        });
        assert!(tokens.stats.had_invariant_violation);
        assert!(!tokens.stats.hit_limit);
        assert_eq!(tokens.stats.diagnostics_emitted, 1);
        assert_eq!(
            tokens.diagnostics.len(),
            usize::from(options.collect_diagnostics && options.limits.max_diagnostics != 0)
        );
        assert!(
            tokens
                .to_debug_snapshot()
                .contains("had_invariant_violation: true")
        );
        // Outer tokens remain valid: only the tokenizer producer raises this flag.
        let syntax = with_fault(Fault::TokenizerTextSpan, || {
            super::parse_stylesheet_with_options("p{color:red}", &options)
        });
        assert!(syntax.stats.had_invariant_violation);
        assert_eq!(syntax.stats.diagnostics_emitted, 1);
        let model = with_fault(Fault::TokenizerTextSpan, || {
            crate::parse_stylesheet_with_options("p{color:red}", &options)
        });
        assert_eq!(
            model.completion(),
            crate::StylesheetParseCompletion::InvariantViolation
        );
        assert!(!model.stats.hit_limit);
        assert_eq!(model.stats.diagnostics_emitted, 1);
        assert!(
            model
                .to_debug_snapshot()
                .contains("had_invariant_violation: true")
        );
    }
    println!("AJ8 integrity tokenizer rows=3");
}
#[test]
fn aj8_integrity_structured_stylesheet_and_declaration_producers() {
    for options in modes() {
        let model = with_fault(Fault::ParserMissingEof, || {
            crate::parse_stylesheet_with_options("p{color:red}", &options)
        });
        assert_eq!(
            model.completion(),
            crate::StylesheetParseCompletion::InvariantViolation
        );
        assert_eq!(model.stats.diagnostics_emitted, 1);
        assert!(!model.stats.hit_limit);
        let mut declaration_options = options.clone();
        declaration_options.origin = super::CssParseOrigin::StyleAttribute;
        for fault in [Fault::TokenizerTextSpan, Fault::ParserMissingEof] {
            let declaration = with_fault(fault, || {
                super::parse_declaration_list_structured("color:red", 17, &declaration_options)
            });
            assert!(declaration.stats.had_invariant_violation);
            assert!(!declaration.stats.hit_limit);
            assert_eq!(declaration.stats.diagnostics_emitted, 1);
            assert_eq!(
                declaration.diagnostics.len(),
                usize::from(options.collect_diagnostics && options.limits.max_diagnostics != 0)
            );
            let model_declaration = with_fault(fault, || {
                crate::model::parse_declaration_list_with_options("color:red", &declaration_options)
            });
            assert!(model_declaration.stats.had_invariant_violation);
        }
    }
    println!("AJ8 integrity structured rows=9");
}
#[test]
fn aj8_integrity_actual_selector_and_semantic_name_producers() {
    for options in modes() {
        let selector = with_fault(Fault::ModelSelector, || {
            crate::parse_stylesheet_with_options("p{color:red}", &options)
        });
        assert_eq!(
            selector.completion(),
            crate::StylesheetParseCompletion::InvariantViolation
        );
        assert!(!selector.stats.hit_limit);
        assert_eq!(selector.stats.diagnostics_emitted, 1);
        assert_eq!(
            selector.diagnostics.len(),
            usize::from(options.collect_diagnostics && options.limits.max_diagnostics != 0)
        );
        let name = with_fault(Fault::ModelAtRuleName, || {
            crate::parse_stylesheet_with_options("@media all {}", &options)
        });
        assert_eq!(
            name.completion(),
            crate::StylesheetParseCompletion::InvariantViolation
        );
        assert!(!name.stats.hit_limit);
        assert_eq!(name.stats.diagnostics_emitted, 0);
        assert!(name.diagnostics.is_empty());
        assert!(!name.diagnostics_omitted());
        assert!(
            name.to_debug_snapshot()
                .contains("had_invariant_violation: true")
        );
    }
    println!("AJ8 integrity model rows=6");
}
#[test]
fn aj8_integrity_real_syntax_fuzz_consumers_reject_internal_faults() {
    let tokenizer = with_fault(Fault::TokenizerTextSpan, || {
        super::run_seeded_tokenizer_fuzz_case(b"p{}", super::CssTokenizerFuzzConfig::default())
    });
    assert!(matches!(
        tokenizer,
        Err(super::CssSyntaxFuzzError::TokenStreamInvariantViolation { .. })
    ));
    for fault in [Fault::TokenizerTextSpan, Fault::ParserMissingEof] {
        let mut config = super::CssParserFuzzConfig::default();
        config.syntax_limits.max_diagnostics = 0;
        let parser = with_fault(fault, || super::run_seeded_parser_fuzz_case(b"p{}", config));
        assert!(
            matches!(parser, Err(super::CssSyntaxFuzzError::StructuralInvariantViolation { phase: "stylesheet parse", detail }) if detail == "parser reported an invariant violation"),
            "syntax fuzz must reject independent integrity evidence"
        );
    }
    println!("AJ8 integrity fuzz rows=3");
}
