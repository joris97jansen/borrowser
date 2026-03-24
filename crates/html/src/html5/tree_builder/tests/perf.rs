use super::helpers::EmptyResolver;
use std::time::Duration;

fn run_tree_builder_chunks(chunks: &[&str]) -> Vec<crate::dom_patch::DomPatch> {
    use crate::html5::shared::{DocumentParseContext, Input};
    use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let mut input = Input::new();

    for chunk in chunks {
        input.push_str(chunk);
        loop {
            let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                assert!(
                    matches!(
                        result,
                        TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                    ),
                    "unexpected tokenizer state while draining chunk: {result:?}"
                );
                break;
            }
            let resolver = batch.resolver();
            for token in batch.iter() {
                let _ = builder
                    .process(token, &ctx.atoms, &resolver)
                    .expect("stress run should remain recoverable");
            }
        }
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            let _ = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("stress EOF drain should remain recoverable");
        }
    }

    builder.drain_patches()
}

fn chunk_slices(input: &str, chunk_size: usize) -> Vec<&str> {
    assert!(chunk_size > 0, "chunk size must be non-zero");
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < input.len() {
        let mut end = (start + chunk_size).min(input.len());
        while end < input.len() && !input.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(&input[start..end]);
        start = end;
    }
    chunks
}

fn assert_no_remove_node_moves(patches: &[crate::dom_patch::DomPatch], context: &str) {
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::RemoveNode { .. })),
        "{context} must keep using canonical AppendChild/InsertBefore move encoding under stress"
    );
}

fn perf_wall_clock_budget(seconds: u64) -> Duration {
    let multiplier = if std::env::var("CI").is_ok() { 3 } else { 1 };
    Duration::from_secs(seconds.saturating_mul(multiplier))
}

fn assert_wall_clock_sanity(elapsed: Duration, base_seconds: u64, context: &str) {
    // Structural counters are the primary perf contract in these tests. Keep a
    // wall-clock sanity bound as a secondary guard, but relax it on contended
    // CI runners where scheduler noise is common.
    let budget = perf_wall_clock_budget(base_seconds);
    assert!(
        elapsed <= budget,
        "{context} took too long: {:?} (budget {:?})",
        elapsed,
        budget
    );
}

fn run_tree_builder_input_for_perf(
    input_html: &str,
) -> crate::html5::tree_builder::Html5TreeBuilder {
    use crate::html5::shared::{DocumentParseContext, Input};
    use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let mut input = Input::new();
    input.push_str(input_html);

    loop {
        let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            assert!(
                matches!(
                    result,
                    TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                ),
                "unexpected tokenizer state while draining whole input: {result:?}"
            );
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            let _ = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("stress parse should remain recoverable");
        }
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            let _ = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("stress EOF drain should remain recoverable");
        }
    }

    builder
}

#[test]
fn tree_builder_perf_sanity_deep_nesting_scope_scan_is_linear_on_typical_path() {
    use crate::html5::shared::Token;
    use std::time::Instant;

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
    assert_wall_clock_sanity(elapsed, 3, "deep nesting stress parse");
}

#[test]
fn tree_builder_perf_sanity_text_coalescing_avoids_quadratic_patch_payload_growth() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};
    use std::time::Instant;

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
    assert_wall_clock_sanity(elapsed, 3, "large text coalescing stress");

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

#[test]
fn tree_builder_perf_sanity_repeated_aaa_misnesting_is_bounded_and_chunk_stable() {
    use std::time::Instant;

    let repeats = 256usize;
    let input = format!("<!doctype html>{}", "<b><i>x</b>y</i>".repeat(repeats));
    let chunked_input = chunk_slices(&input, 31);

    let started = Instant::now();
    let whole = run_tree_builder_chunks(&[input.as_str()]);
    let chunked = run_tree_builder_chunks(&chunked_input);
    let elapsed = started.elapsed();

    assert_eq!(
        whole, chunked,
        "repeated AAA misnesting stress must preserve exact patch order and key allocation across chunking"
    );
    assert_no_remove_node_moves(&whole, "repeated AAA misnesting stress");
    assert_wall_clock_sanity(elapsed, 5, "repeated AAA misnesting stress");
}

#[test]
fn tree_builder_perf_sanity_reconstruction_plus_aaa_is_bounded_and_chunk_stable() {
    use std::time::Instant;

    let repeats = 128usize;
    let input = format!(
        "<!doctype html>{}",
        "<div><b><i>x</div>y</b></i>".repeat(repeats)
    );
    let chunked_input = chunk_slices(&input, 29);

    let started = Instant::now();
    let whole = run_tree_builder_chunks(&[input.as_str()]);
    let chunked = run_tree_builder_chunks(&chunked_input);
    let elapsed = started.elapsed();

    assert_eq!(
        whole, chunked,
        "repeated reconstruction+AAA stress must preserve exact patch order and key allocation across chunking"
    );
    assert_no_remove_node_moves(&whole, "repeated reconstruction+AAA stress");
    assert_wall_clock_sanity(elapsed, 5, "repeated reconstruction+AAA stress");
}

#[test]
fn tree_builder_perf_sanity_deep_table_foster_parenting_uses_amortized_anchor_scans() {
    use std::time::Instant;

    fn make_input(depth: usize, stray_divs: usize) -> String {
        let mut input = String::from("<!doctype html>");
        for _ in 0..depth {
            input.push_str("<table><tr><td>");
        }
        input.push_str("<table>");
        for _ in 0..stray_divs {
            input.push_str("<div>x</div>");
        }
        input.push_str("</table>");
        for _ in 0..depth {
            input.push_str("</td></tr></table>");
        }
        input
    }

    let depth = 256usize;
    let stray_divs = 1_024usize;
    let input = make_input(depth, stray_divs);

    let started = Instant::now();
    let mut builder = run_tree_builder_input_for_perf(&input);
    let elapsed = started.elapsed();

    assert!(
        builder.open_elements.foster_parenting_scan_calls() <= 4,
        "repeated foster-parented misplaced tags should reuse cached table/html anchors; calls={}",
        builder.open_elements.foster_parenting_scan_calls()
    );
    assert!(
        builder.open_elements.foster_parenting_scan_steps() <= (depth as u64) * 4 + 16,
        "foster-parent anchor rescans should stay proportional to table depth, not misplaced-token count; steps={} depth={depth}",
        builder.open_elements.foster_parenting_scan_steps()
    );

    let patches = builder.drain_patches();
    assert_no_remove_node_moves(&patches, "deep table foster-parenting stress");
    assert_wall_clock_sanity(elapsed, 5, "deep table foster-parenting stress");
}
