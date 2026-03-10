use super::helpers::EmptyResolver;

#[test]
fn tree_builder_perf_sanity_deep_nesting_scope_scan_is_linear_on_typical_path() {
    use crate::html5::shared::Token;
    use std::time::{Duration, Instant};

    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let resolver = EmptyResolver;
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    let depth = 4_096usize;
    let started = Instant::now();
    for _ in 0..depth {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: div,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("start-tag should process");
    }
    for _ in 0..depth {
        let _ = builder
            .process(&Token::EndTag { name: div }, &ctx.atoms, &resolver)
            .expect("end-tag should process");
    }
    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("eof should process");
    let elapsed = started.elapsed();

    let stats = builder.debug_perf_stats();
    assert!(
        stats.soe_scope_scan_calls >= depth as u64,
        "expected at least one scope-scan call per close on deep nesting"
    );
    assert!(
        stats.soe_scope_scan_steps <= (depth as u64) * 3,
        "common close-on-top path should stay near O(1) scan per end tag; steps={} depth={depth}",
        stats.soe_scope_scan_steps
    );
    assert!(
        elapsed <= Duration::from_secs(3),
        "deep nesting stress parse took too long: {:?}",
        elapsed
    );
}

#[test]
fn tree_builder_perf_sanity_text_coalescing_avoids_quadratic_patch_payload_growth() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};
    use std::time::{Duration, Instant};

    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig {
            coalesce_text: true,
        },
        &mut ctx,
    )
    .expect("tree builder init");
    let resolver = EmptyResolver;
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    let text_tokens = 20_000usize;
    let started = Instant::now();
    let _ = builder
        .process(
            &Token::StartTag {
                name: div,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("start-tag should process");
    for _ in 0..text_tokens {
        let _ = builder
            .process(
                &Token::Text {
                    text: TextValue::Owned("x".to_string()),
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("text should process");
    }
    let _ = builder
        .process(&Token::EndTag { name: div }, &ctx.atoms, &resolver)
        .expect("end-tag should process");
    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("eof should process");
    let elapsed = started.elapsed();

    let stats = builder.debug_perf_stats();
    assert_eq!(
        stats.text_nodes_created, 1,
        "adjacent text should create one text node under coalescing"
    );
    assert_eq!(
        stats.text_appends,
        (text_tokens - 1) as u64,
        "remaining text tokens should emit incremental AppendText patches"
    );
    assert!(
        elapsed <= Duration::from_secs(3),
        "large text coalescing stress took too long: {:?}",
        elapsed
    );

    let patches = builder.drain_patches();
    let create_text_count = patches
        .iter()
        .filter(|patch| matches!(patch, DomPatch::CreateText { .. }))
        .count();
    let append_text_count = patches
        .iter()
        .filter(|patch| matches!(patch, DomPatch::AppendText { .. }))
        .count();
    assert_eq!(create_text_count, 1);
    assert_eq!(append_text_count, text_tokens - 1);
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::SetText { .. })),
        "coalescing path should not regress to cumulative SetText payload growth"
    );
    let append_total_bytes: usize = patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::AppendText { text, .. } => Some(text.len()),
            _ => None,
        })
        .sum();
    let append_max_bytes: usize = patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::AppendText { text, .. } => Some(text.len()),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert_eq!(append_total_bytes, text_tokens - 1);
    assert_eq!(
        append_max_bytes, 1,
        "append payload should remain token-local, not cumulative"
    );
}
