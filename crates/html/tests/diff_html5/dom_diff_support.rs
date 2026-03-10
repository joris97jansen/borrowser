use super::{CaseContext, ensure_need_more_input_only_at_buffer_end, validate_tokenize_result};
use html::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, VecPatchSink};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};

struct PatchCollectionCtx<'a> {
    tokenizer: &'a mut Html5Tokenizer,
    input: &'a mut Input,
    builder: &'a mut Html5TreeBuilder,
    parse_ctx: &'a DocumentParseContext,
    patch_batches: &'a mut Vec<Vec<html::DomPatch>>,
    saw_eof_token: &'a mut bool,
}

pub(super) struct Html5DomDriver<'a> {
    case: CaseContext<'a>,
}

impl<'a> Html5DomDriver<'a> {
    pub(super) fn new(case: CaseContext<'a>) -> Self {
        Self { case }
    }

    pub(super) fn materialize_html5_dom_via_patches(
        &self,
        input_html: &str,
    ) -> Result<html::Node, String> {
        let mut ctx = DocumentParseContext::new();
        let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
        let mut builder =
            Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).map_err(|err| {
                format!(
                    "failed to init tree builder in '{}' at {:?}: {err:?}",
                    self.case.id, self.case.path
                )
            })?;
        let mut input = Input::new();
        let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
        let mut saw_eof_token = false;

        input.push_str(input_html);
        loop {
            let consumed_before = tokenizer.stats().bytes_consumed;
            let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let consumed_after = tokenizer.stats().bytes_consumed;
            validate_tokenize_result(result, "push_input").map_err(|err| {
                format!(
                    "tokenizer error in '{}' at {:?}: {err}",
                    self.case.id, self.case.path
                )
            })?;
            ensure_need_more_input_only_at_buffer_end(
                self.case,
                result,
                consumed_before,
                consumed_after,
                input.as_str().len(),
            )?;
            self.collect_patch_batches(
                PatchCollectionCtx {
                    tokenizer: &mut tokenizer,
                    input: &mut input,
                    builder: &mut builder,
                    parse_ctx: &ctx,
                    patch_batches: &mut patch_batches,
                    saw_eof_token: &mut saw_eof_token,
                },
                true,
            )?;
            if matches!(result, TokenizeResult::NeedMoreInput) {
                break;
            }
        }

        validate_tokenize_result(tokenizer.finish(&input), "finish").map_err(|err| {
            format!(
                "tokenizer error in '{}' at {:?}: {err}",
                self.case.id, self.case.path
            )
        })?;
        self.collect_patch_batches(
            PatchCollectionCtx {
                tokenizer: &mut tokenizer,
                input: &mut input,
                builder: &mut builder,
                parse_ctx: &ctx,
                patch_batches: &mut patch_batches,
                saw_eof_token: &mut saw_eof_token,
            },
            false,
        )?;
        if !saw_eof_token {
            return Err(format!(
                "expected EOF token but none was observed (case '{}' at {:?})",
                self.case.id, self.case.path
            ));
        }
        html::test_harness::materialize_patch_batches(&patch_batches)
    }

    fn collect_patch_batches(
        &self,
        ctx: PatchCollectionCtx<'_>,
        expect_token_granular_batches: bool,
    ) -> Result<(), String> {
        let PatchCollectionCtx {
            tokenizer,
            input,
            builder,
            parse_ctx,
            patch_batches,
            saw_eof_token,
        } = ctx;
        let mut patches = Vec::new();
        loop {
            let batch = tokenizer.next_batch(input);
            if batch.tokens().is_empty() {
                break;
            }
            if expect_token_granular_batches {
                assert_eq!(
                    batch.tokens().len(),
                    1,
                    "tokenizer control-aware tree-builder driver must observe exactly one token per pump"
                );
            }
            patches.clear();
            let resolver = batch.resolver();
            let atoms = &parse_ctx.atoms;
            let mut sink = VecPatchSink(&mut patches);
            for token in batch.iter() {
                if matches!(token, html::html5::Token::Eof) {
                    *saw_eof_token = true;
                }
                match builder.push_token(token, atoms, &resolver, &mut sink) {
                    Ok(step) => {
                        if let Some(control) = step.tokenizer_control {
                            tokenizer.apply_control(control);
                        }
                        if let html::html5::TreeBuilderControlFlow::Suspend(reason) = step.flow {
                            return Err(format!("tree builder suspended: {reason:?}"));
                        }
                    }
                    Err(err) => {
                        return Err(format!("tree builder error: {err:?}"));
                    }
                }
            }
            if !patches.is_empty() {
                patch_batches.push(std::mem::take(&mut patches));
            }
        }
        Ok(())
    }
}
