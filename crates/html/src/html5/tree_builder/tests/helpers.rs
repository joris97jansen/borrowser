pub(super) struct EmptyResolver;

impl crate::html5::tokenizer::TextResolver for EmptyResolver {
    fn resolve_span(
        &self,
        span: crate::html5::shared::TextSpan,
    ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
        Err(crate::html5::tokenizer::TextResolveError::InvalidSpan { span })
    }
}

pub(super) fn enter_after_head(
    builder: &mut crate::html5::tree_builder::Html5TreeBuilder,
    ctx: &mut crate::html5::shared::DocumentParseContext,
    resolver: &EmptyResolver,
) -> Vec<crate::dom_patch::DomPatch> {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");

    for token in [
        Token::Doctype {
            name: Some(html),
            public_id: None,
            system_id: None,
            force_quirks: false,
        },
        Token::StartTag {
            name: html,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: head,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::EndTag { name: head },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, resolver)
            .expect("after-head prelude should process");
    }
    let snap = builder.state_snapshot();
    assert_eq!(
        snap.insertion_mode,
        InsertionMode::AfterHead,
        "enter_after_head() must leave builder in AfterHead"
    );
    assert!(
        snap.open_element_names.contains(&html),
        "expected <html> on SOE after enter_after_head()"
    );
    assert!(
        !snap.open_element_names.contains(&head),
        "expected <head> to be popped from SOE after enter_after_head()"
    );
    assert_eq!(
        snap.quirks_mode,
        crate::html5::tree_builder::QuirksMode::NoQuirks,
        "enter_after_head() should keep NoQuirks for a normal html doctype"
    );
    let errors = builder.take_parse_error_kinds_for_test();
    // Core-v0: Initial mode may still report a placeholder error here.
    // Once DOCTYPE handling in Initial is fully wired, this should become error-free.
    assert!(
        errors.is_empty()
            || errors
                .iter()
                .copied()
                .all(|kind| kind == "initial-unexpected-token"),
        "enter_after_head() prelude reported unexpected parse errors: {errors:?}"
    );
    builder.drain_patches()
}

pub(super) fn panic_message_contains(payload: &(dyn std::any::Any + Send), needle: &str) -> bool {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        return msg.contains(needle);
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.contains(needle);
    }
    false
}

pub(super) fn assert_binding_mismatch_panic(payload: &(dyn std::any::Any + Send), context: &str) {
    assert!(
        panic_message_contains(payload, "tree builder atom table mismatch"),
        "{context} panic must come from atom-table binding assertion"
    );
    assert!(
        panic_message_contains(payload, "expected=") && panic_message_contains(payload, "actual="),
        "{context} panic should include expected/actual atom table ids"
    );
}
