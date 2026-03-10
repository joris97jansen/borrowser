use super::helpers::{EmptyResolver, assert_binding_mismatch_panic};

#[test]
fn tree_builder_rejects_foreign_atom_table() {
    use crate::html5::shared::Token;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let resolver = EmptyResolver;
    let mut owner_ctx = crate::html5::shared::DocumentParseContext::new();
    let foreign_ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut owner_ctx,
    )
    .expect("tree builder init");

    let process_panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = builder.process(&Token::Eof, &foreign_ctx.atoms, &resolver);
    }))
    .expect_err("process must trip invariant assertion");
    assert_binding_mismatch_panic(process_panic.as_ref(), "process");

    let push_panic = catch_unwind(AssertUnwindSafe(|| {
        let mut out = Vec::new();
        let mut sink = crate::html5::tree_builder::VecPatchSink(&mut out);
        let _ = builder.push_token(&Token::Eof, &foreign_ctx.atoms, &resolver, &mut sink);
    }))
    .expect_err("push_token must trip invariant assertion");
    assert_binding_mismatch_panic(push_panic.as_ref(), "push_token");

    let recovery_result = builder.process(&Token::Eof, &owner_ctx.atoms, &resolver);
    assert!(
        recovery_result.is_ok(),
        "builder should remain usable with its bound atom table after rejection"
    );
}
