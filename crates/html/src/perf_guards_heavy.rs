use crate::conformance::{
    CanonicalParserResult, EventPosition, FinalInvariantRequest, ObservationRequest,
    ObservationState, ObservedInsertionMode, ParseErrorCode, ParseErrorEvent, ParserContextSummary,
    ParserObservationInput, ParserObservationRequest, ParserObservationTarget,
    ParserRecoveryAction, ParserStage, ParserTokenKind, PositionUnavailableReason,
    ScalarObservationRequest, TreeConstructionParseErrorCode, execute_parser_observation,
};
use crate::perf_fixtures::make_blocks;
use crate::{DocumentMode, HtmlParseCounters, HtmlParseOptions, HtmlParser, Node, parse_document};

const LARGE_BLOCKS: usize = 5_000;
const LARGE_BLOCK_TOKENS: u64 = 6;
const EOF_TOKENS: u64 = 1;
const PATCHES_PER_BLOCK: usize = 8;
const HTML5_DOCUMENT_BOOTSTRAP_NODES: usize = 4;
const HTML5_DOCUMENT_BOOTSTRAP_PATCHES: usize = 7;
const MAX_TOKENS_PER_BYTE_LARGE: f64 = 0.20;
const MAX_PATCHES_PER_BYTE_LARGE: f64 = 0.30;
const MAX_TOKENS_PER_BYTE_RAWTEXT: f64 = 0.01;
const MAX_PATCHES_PER_BYTE_RAWTEXT: f64 = 0.01;
const CHUNK_SCHEDULES: &[&[usize]] = &[&[1], &[1, 2, 3, 7, 64, 128, 256], &[64], &[1_024]];

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParseMetrics {
    counters: HtmlParseCounters,
    patch_count: usize,
    node_count: usize,
    parse_error_count: usize,
}

fn make_rawtext_adversarial(bytes: usize) -> String {
    let mut body = String::with_capacity(bytes + 32);
    body.push_str("<script>");
    let target_len = body.len().saturating_add(bytes);
    while body.len() < target_len {
        body.push_str("</scri");
        body.push('<');
        body.push_str("pt");
    }
    body.push_str("</script>");
    body
}

fn node_count(node: &Node) -> usize {
    crate::traverse::full_model_node_count(node)
}

fn text_node_count(node: &Node) -> usize {
    crate::traverse::full_model_preorder(node)
        .filter(|visit| {
            matches!(
                visit.entry,
                crate::traverse::FullModelNodeRef::Node(Node::Text { .. })
            )
        })
        .count()
}

fn total_text_bytes(node: &Node) -> usize {
    crate::traverse::full_model_preorder(node)
        .filter_map(|visit| match visit.entry {
            crate::traverse::FullModelNodeRef::Node(Node::Text { text, .. }) => Some(text.len()),
            _ => None,
        })
        .sum()
}

fn assert_missing_doctype_accounting(metrics: &ParseMetrics, label: &str) {
    // The missing-doctype tree error is counted but has no source position,
    // so it cannot appear in the legacy positional-event vector. The canonical
    // fixture regression verifies the diagnostic's identity separately.
    assert_eq!(
        metrics.counters.parse_errors, 1,
        "expected only the missing-doctype tree error for {label}"
    );
    assert_eq!(
        metrics.counters.errors_dropped, 0,
        "unexpected dropped parse errors for {label}"
    );
    assert_eq!(
        metrics.parse_error_count, 0,
        "unexpected surfaced parse errors for {label}"
    );
}

fn parse_chunked(input: &str, chunk_sizes: &[usize]) -> ParseMetrics {
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("chunked html5 parser init");
    let bytes = input.as_bytes();
    let mut offset = 0usize;
    let mut size_index = 0usize;
    let mut drained_patch_count = 0usize;
    while offset < bytes.len() {
        let size = chunk_sizes[size_index % chunk_sizes.len()];
        let end = (offset + size).min(bytes.len());
        parser
            .push_bytes(&bytes[offset..end])
            .expect("chunk push should succeed");
        parser.pump().expect("chunk pump should succeed");
        drained_patch_count = drained_patch_count.saturating_add(
            parser
                .take_patches()
                .expect("chunk patch drain should succeed")
                .len(),
        );
        offset = end;
        size_index += 1;
    }
    parser.finish().expect("chunked finish should succeed");
    drained_patch_count = drained_patch_count.saturating_add(
        parser
            .take_patches()
            .expect("final patch drain should succeed")
            .len(),
    );
    let output = parser
        .into_output()
        .expect("chunked output should materialize");
    assert_eq!(output.document_mode, DocumentMode::Quirks);
    ParseMetrics {
        counters: output.counters,
        patch_count: drained_patch_count.saturating_add(output.patches.len()),
        node_count: node_count(&output.document),
        parse_error_count: output.parse_errors.len(),
    }
}

fn observe_perf_fixture(input: ParserObservationInput<'_>) -> CanonicalParserResult {
    execute_parser_observation(ParserObservationRequest {
        target: ParserObservationTarget::DocumentParser,
        input,
        tokens: ObservationRequest::Capture {
            capacity: LARGE_BLOCKS * LARGE_BLOCK_TOKENS as usize + EOF_TOKENS as usize,
        },
        parse_errors: ObservationRequest::Capture { capacity: 1 },
        implementation_diagnostics: ObservationRequest::Capture { capacity: 1 },
        transitions: ObservationRequest::NotRequested,
        unsupported_features: ObservationRequest::NotRequested,
        document_mode: ScalarObservationRequest::Capture,
        tree: ObservationRequest::Capture {
            capacity: HTML5_DOCUMENT_BOOTSTRAP_NODES + LARGE_BLOCKS * 4,
        },
        patches: ObservationRequest::Capture {
            capacity: HTML5_DOCUMENT_BOOTSTRAP_PATCHES + LARGE_BLOCKS * PATCHES_PER_BLOCK,
        },
        final_invariants: FinalInvariantRequest::NotRequested,
    })
    .expect("performance fixture canonical parse should succeed")
}

fn assert_only_expected_doctype_error(result: &CanonicalParserResult, label: &str) {
    let ObservationState::Captured(errors) = &result.parse_errors else {
        panic!("missing or incomplete parse-error capture for {label}");
    };
    assert!(
        matches!(
            errors.as_slice(),
            [ParseErrorEvent {
                occurrence: 1,
                stage: ParserStage::TreeConstruction,
                code: ParseErrorCode::TreeConstruction(
                    TreeConstructionParseErrorCode::ExpectedDoctypeBeforeNonSpaceToken
                ),
                recovery: Some(ParserRecoveryAction::ReprocessToken),
                position: EventPosition::Unavailable(
                    PositionUnavailableReason::ParserDidNotProvidePosition
                ),
                context: Some(ParserContextSummary {
                    token_kind: Some(ParserTokenKind::StartTag),
                    insertion_mode: Some(ObservedInsertionMode::Initial),
                    adjusted_current_node_namespace: None,
                }),
                ..
            }]
        ),
        "unexpected parse errors for {label}: {errors:?}"
    );
    assert!(
        matches!(
            &result.implementation_diagnostics,
            ObservationState::Captured(diagnostics) if diagnostics.is_empty()
        ),
        "expected complete, empty implementation diagnostics for {label}"
    );
    assert_eq!(
        result.document_mode,
        ObservationState::Captured(DocumentMode::Quirks),
        "unexpected document mode for {label}"
    );
    assert!(
        matches!(&result.tokens, ObservationState::Captured(_)),
        "missing or incomplete token capture for {label}"
    );
    assert!(
        matches!(&result.tree, ObservationState::Captured(_)),
        "missing or incomplete tree capture for {label}"
    );
    assert!(
        matches!(&result.patches, ObservationState::Captured(_)),
        "missing or incomplete patch capture for {label}"
    );
}

#[test]
fn perf_guard_fixtures_report_only_expected_doctype_error() {
    for (label, input, chunk_schedules) in [
        ("chunked HTML", make_blocks(1_000), CHUNK_SCHEDULES),
        ("large HTML", make_blocks(LARGE_BLOCKS), &[][..]),
        (
            "rawtext adversarial HTML",
            make_rawtext_adversarial(256 * 1024),
            &[][..],
        ),
    ] {
        let whole = observe_perf_fixture(ParserObservationInput::Bytes(input.as_bytes()));
        assert_only_expected_doctype_error(&whole, label);

        for &chunk_sizes in chunk_schedules {
            let mut byte_offsets = Vec::new();
            let mut offset = 0usize;
            for &size in chunk_sizes.iter().cycle() {
                offset = (offset + size).min(input.len());
                if offset == input.len() {
                    break;
                }
                byte_offsets.push(offset);
            }
            let streamed = observe_perf_fixture(ParserObservationInput::ByteBoundaryChunks {
                bytes: input.as_bytes(),
                byte_offsets: &byte_offsets,
            });
            assert_only_expected_doctype_error(&streamed, label);
            assert_eq!(
                streamed, whole,
                "chunk sizes {chunk_sizes:?} changed canonical tokens, tree, patches, diagnostics or document mode for {label}"
            );
        }
    }
}

#[test]
fn perf_guard_parse_large_token_patch_and_node_counts() {
    let input = make_blocks(LARGE_BLOCKS);
    let output =
        parse_document(&input, HtmlParseOptions::default()).expect("large html5 parse should work");
    assert_eq!(output.document_mode, DocumentMode::Quirks);
    let metrics = ParseMetrics {
        counters: output.counters.clone(),
        patch_count: output.patches.len(),
        node_count: node_count(&output.document),
        parse_error_count: output.parse_errors.len(),
    };
    assert_missing_doctype_accounting(&metrics, "large HTML");

    let expected_tokens = (LARGE_BLOCKS as u64 * LARGE_BLOCK_TOKENS) + EOF_TOKENS;
    assert_eq!(
        metrics.counters.tokens_processed, expected_tokens,
        "unexpected token count for large HTML"
    );
    let token_ratio = metrics.counters.tokens_processed as f64 / input.len() as f64;
    assert!(
        token_ratio <= MAX_TOKENS_PER_BYTE_LARGE,
        "token/byte ratio {token_ratio:.4} exceeded guard {MAX_TOKENS_PER_BYTE_LARGE}"
    );

    let expected_patches = HTML5_DOCUMENT_BOOTSTRAP_PATCHES + (LARGE_BLOCKS * PATCHES_PER_BLOCK);
    assert_eq!(
        metrics.patch_count, expected_patches,
        "unexpected patch count for large HTML"
    );
    let patch_ratio = metrics.patch_count as f64 / input.len() as f64;
    assert!(
        patch_ratio <= MAX_PATCHES_PER_BYTE_LARGE,
        "patch/byte ratio {patch_ratio:.4} exceeded guard {MAX_PATCHES_PER_BYTE_LARGE}"
    );

    let expected_nodes = HTML5_DOCUMENT_BOOTSTRAP_NODES + (LARGE_BLOCKS * 4);
    assert_eq!(
        metrics.node_count, expected_nodes,
        "unexpected node count for large HTML"
    );
}

#[test]
fn perf_guard_rawtext_scan_metrics_stay_linear() {
    let input = make_rawtext_adversarial(256 * 1024);
    let output = parse_document(&input, HtmlParseOptions::default())
        .expect("rawtext adversarial parse should work");
    assert_eq!(output.document_mode, DocumentMode::Quirks);
    let metrics = ParseMetrics {
        counters: output.counters.clone(),
        patch_count: output.patches.len(),
        node_count: node_count(&output.document),
        parse_error_count: output.parse_errors.len(),
    };
    assert_missing_doctype_accounting(&metrics, "rawtext adversarial HTML");

    let token_ratio = metrics.counters.tokens_processed as f64 / input.len() as f64;
    assert!(
        token_ratio <= MAX_TOKENS_PER_BYTE_RAWTEXT,
        "rawtext token/byte ratio {token_ratio:.6} exceeded guard {MAX_TOKENS_PER_BYTE_RAWTEXT}"
    );
    let patch_ratio = metrics.patch_count as f64 / input.len() as f64;
    assert!(
        patch_ratio <= MAX_PATCHES_PER_BYTE_RAWTEXT,
        "rawtext patch/byte ratio {patch_ratio:.6} exceeded guard {MAX_PATCHES_PER_BYTE_RAWTEXT}"
    );
    assert_eq!(
        text_node_count(&output.document),
        1,
        "rawtext adversarial parse should materialize a single text node"
    );
    assert!(
        total_text_bytes(&output.document) >= 256 * 1024,
        "rawtext adversarial parse should preserve the large literal text payload"
    );
}

#[test]
fn perf_guard_chunk_sizes_preserve_html5_work_metrics() {
    let input = make_blocks(1_000);
    let full_output =
        parse_document(&input, HtmlParseOptions::default()).expect("whole parse should work");
    assert_eq!(full_output.document_mode, DocumentMode::Quirks);
    let full_metrics = ParseMetrics {
        counters: full_output.counters.clone(),
        patch_count: full_output.patches.len(),
        node_count: node_count(&full_output.document),
        parse_error_count: full_output.parse_errors.len(),
    };
    assert_missing_doctype_accounting(&full_metrics, "whole HTML");

    for &chunk_sizes in CHUNK_SCHEDULES {
        let metrics = parse_chunked(&input, chunk_sizes);
        assert_eq!(
            metrics, full_metrics,
            "chunk sizes {chunk_sizes:?} changed html5 performance counters or structural totals"
        );
    }
}
