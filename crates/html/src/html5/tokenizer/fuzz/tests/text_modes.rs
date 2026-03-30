use super::super::config::{TokenizerFuzzConfig, TokenizerFuzzTermination, derive_fuzz_seed};
use super::super::driver::{
    run_seeded_rawtext_fuzz_case, run_seeded_textarea_rcdata_fuzz_case,
    run_seeded_title_rcdata_fuzz_case,
};
use crate::html5::shared::{
    ByteStreamDecoder, DecodeResult, DocumentParseContext, Input, TextValue, Token,
};
use crate::html5::tokenizer::{
    HarnessRng, Html5Tokenizer, TextModeSpec, TextResolver, TokenizeResult, TokenizerConfig,
    TokenizerControl, next_chunk_len,
};

#[test]
fn seeded_rawtext_fuzz_harness_is_reproducible() {
    let bytes = b"</stylX><<<<</style \t>";
    let config = TokenizerFuzzConfig {
        seed: 0x7171,
        max_chunk_len: 11,
        ..TokenizerFuzzConfig::default()
    };
    let first = run_seeded_rawtext_fuzz_case(bytes, config).expect("first run should complete");
    let second = run_seeded_rawtext_fuzz_case(bytes, config).expect("second run should complete");
    assert_eq!(first, second);
    assert_eq!(first.termination, TokenizerFuzzTermination::Completed);
    assert!(first.saw_one_byte_chunk);
}

#[test]
fn seeded_rawtext_fuzz_harness_handles_dense_lt_near_miss_storm() {
    let mut bytes = Vec::new();
    for _ in 0..2048 {
        bytes.extend_from_slice(b"<<</stylX");
    }
    bytes.extend_from_slice(b"</style>");

    let summary = run_seeded_rawtext_fuzz_case(
        &bytes,
        TokenizerFuzzConfig {
            seed: derive_fuzz_seed(&bytes),
            max_chunk_len: 19,
            max_input_bytes: 64 * 1024,
            max_decoded_bytes: 256 * 1024,
            ..TokenizerFuzzConfig::default()
        },
    )
    .expect("hostile rawtext close-tag storm should remain bounded");
    assert_eq!(summary.termination, TokenizerFuzzTermination::Completed);
    assert!(summary.saw_one_byte_chunk);
    assert!(summary.chunk_count > 1);
}

#[test]
fn seeded_rcdata_fuzz_harness_is_reproducible_for_title_and_textarea() {
    let bytes = b"A&amp;B</titleX></textareaX><<<</title></textarea>";
    let config = TokenizerFuzzConfig {
        seed: 0x8181,
        max_chunk_len: 13,
        ..TokenizerFuzzConfig::default()
    };

    let title_first =
        run_seeded_title_rcdata_fuzz_case(bytes, config).expect("title run should complete");
    let title_second =
        run_seeded_title_rcdata_fuzz_case(bytes, config).expect("title replay should complete");
    let textarea_first =
        run_seeded_textarea_rcdata_fuzz_case(bytes, config).expect("textarea run should complete");
    let textarea_second = run_seeded_textarea_rcdata_fuzz_case(bytes, config)
        .expect("textarea replay should complete");

    assert_eq!(title_first, title_second);
    assert_eq!(textarea_first, textarea_second);
    assert_eq!(title_first.termination, TokenizerFuzzTermination::Completed);
    assert_eq!(
        textarea_first.termination,
        TokenizerFuzzTermination::Completed
    );
}

#[test]
fn rcdata_decodes_entities_while_rawtext_keeps_them_literal() {
    let rawtext_bytes = b"A&amp;B</style>";
    let title_rcdata_bytes = b"A&amp;B</title>";
    let textarea_rcdata_bytes = b"A&amp;B</textarea>";

    let rawtext_text = collect_text_payloads_seeded(rawtext_bytes, TextProbeMode::RawtextStyle);
    let title_rcdata_text =
        collect_text_payloads_seeded(title_rcdata_bytes, TextProbeMode::RcdataTitle);
    let textarea_rcdata_text =
        collect_text_payloads_seeded(textarea_rcdata_bytes, TextProbeMode::RcdataTextarea);

    assert_eq!(
        rawtext_text,
        vec!["A&amp;B".to_string()],
        "RAWTEXT must keep entity-looking text literal"
    );
    assert_eq!(
        title_rcdata_text,
        vec!["A&B".to_string()],
        "RCDATA title must decode character references"
    );
    assert_eq!(
        textarea_rcdata_text,
        vec!["A&B".to_string()],
        "RCDATA textarea must decode character references"
    );
}

#[derive(Clone, Copy)]
enum TextProbeMode {
    RawtextStyle,
    RcdataTitle,
    RcdataTextarea,
}

impl TextProbeMode {
    fn tag_name(self) -> &'static str {
        match self {
            Self::RawtextStyle => "style",
            Self::RcdataTitle => "title",
            Self::RcdataTextarea => "textarea",
        }
    }

    fn spec(self, tag_name: crate::html5::shared::AtomId) -> TextModeSpec {
        match self {
            Self::RawtextStyle => TextModeSpec::rawtext_style(tag_name),
            Self::RcdataTitle => TextModeSpec::rcdata_title(tag_name),
            Self::RcdataTextarea => TextModeSpec::rcdata_textarea(tag_name),
        }
    }
}

fn collect_text_payloads_seeded(bytes: &[u8], mode: TextProbeMode) -> Vec<String> {
    let mut ctx = DocumentParseContext::new();
    let tag_name = ctx
        .atoms
        .intern_ascii_folded(mode.tag_name())
        .expect("text probe atom interning");
    let spec = mode.spec(tag_name);
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    tokenizer.apply_control(TokenizerControl::EnterTextMode(spec));
    let mut controller = TextProbeController::new(spec);
    let mut input = Input::new();
    let mut decoder = ByteStreamDecoder::new();
    let mut rng = HarnessRng::new(derive_fuzz_seed(bytes));
    let mut chunk_index = 0usize;
    let mut offset = 0usize;
    let mut out = Vec::new();

    while offset < bytes.len() {
        let chunk_len = next_chunk_len(bytes.len() - offset, chunk_index, 8, &mut rng);
        decoder.push_bytes(&bytes[offset..offset + chunk_len], &mut input);
        offset += chunk_len;
        chunk_index = chunk_index.saturating_add(1);
        pump_collecting_text(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut controller,
            &mut out,
        );
    }

    if matches!(decoder.finish(&mut input), DecodeResult::Progress) {
        pump_collecting_text(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut controller,
            &mut out,
        );
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let mut pending_control = None;
        {
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                break;
            }
            let resolver = batch.resolver();
            for token in batch.iter() {
                if let Token::Text { text } = token {
                    out.push(resolve_text_value(text, &resolver));
                }
                pending_control = controller.note_token(token);
            }
        }
        if let Some(control) = pending_control {
            tokenizer.apply_control(control);
            tokenizer
                .check_invariants(&input)
                .expect("tokenizer invariants must hold after finish-time control");
        }
    }

    out
}

fn pump_collecting_text(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    controller: &mut TextProbeController,
    out: &mut Vec<String>,
) {
    loop {
        let result = tokenizer.push_input_until_token(input, ctx);
        tokenizer
            .check_invariants(input)
            .expect("tokenizer invariants must hold during text probe");
        let mut pending_control = None;
        {
            let batch = tokenizer.next_batch(input);
            assert!(
                batch.tokens().len() <= 1,
                "token-granular text probe expected at most one newly emitted token per pump"
            );
            let resolver = batch.resolver();
            for token in batch.iter() {
                if let Token::Text { text } = token {
                    out.push(resolve_text_value(text, &resolver));
                }
                pending_control = controller.note_token(token);
            }
        }
        if let Some(control) = pending_control {
            tokenizer.apply_control(control);
            tokenizer
                .check_invariants(input)
                .expect("tokenizer invariants must hold after text probe control");
        }
        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
}

fn resolve_text_value(text: &TextValue, resolver: &dyn TextResolver) -> String {
    match text {
        TextValue::Span(span) => resolver
            .resolve_span(*span)
            .expect("text probe spans must resolve")
            .to_string(),
        TextValue::Owned(text) => text.clone(),
    }
}

struct TextProbeController {
    spec: TextModeSpec,
    text_mode_active: bool,
}

impl TextProbeController {
    fn new(spec: TextModeSpec) -> Self {
        Self {
            spec,
            text_mode_active: true,
        }
    }

    fn note_token(&mut self, token: &Token) -> Option<TokenizerControl> {
        match token {
            Token::StartTag { name, .. }
                if *name == self.spec.end_tag_name && !self.text_mode_active =>
            {
                self.text_mode_active = true;
                Some(TokenizerControl::EnterTextMode(self.spec))
            }
            Token::EndTag { name } if *name == self.spec.end_tag_name && self.text_mode_active => {
                self.text_mode_active = false;
                Some(TokenizerControl::ExitTextMode)
            }
            _ => None,
        }
    }
}
