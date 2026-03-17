use super::super::helpers::EmptyResolver;
use crate::dom_patch::DomPatch;
use crate::html5::shared::{AtomId, DocumentParseContext, Token};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult};

#[derive(Clone, Copy)]
pub(super) enum Prelude {
    Head,
    Body,
}

pub(super) struct EnteredTextMode {
    pub(super) tag: AtomId,
}

pub(super) struct TextModeHarness {
    resolver: EmptyResolver,
    ctx: DocumentParseContext,
    builder: Html5TreeBuilder,
}

impl TextModeHarness {
    pub(super) fn new() -> Self {
        Self::with_config(TreeBuilderConfig::default())
    }

    pub(super) fn with_config(config: TreeBuilderConfig) -> Self {
        let resolver = EmptyResolver;
        let mut ctx = DocumentParseContext::new();
        let builder = Html5TreeBuilder::new(config, &mut ctx).expect("tree builder init");
        Self {
            resolver,
            ctx,
            builder,
        }
    }

    pub(super) fn atom(&mut self, name: &str) -> AtomId {
        self.ctx
            .atoms
            .intern_ascii_folded(name)
            .expect("atom interning")
    }

    pub(super) fn process(&mut self, token: Token) -> TreeBuilderStepResult {
        self.builder
            .process(&token, &self.ctx.atoms, &self.resolver)
            .expect("text-mode test token should remain recoverable")
    }

    pub(super) fn process_ok(&mut self, token: Token) {
        let _ = self.process(token);
    }

    pub(super) fn process_all(&mut self, tokens: impl IntoIterator<Item = Token>) {
        for token in tokens {
            self.process_ok(token);
        }
    }

    pub(super) fn prelude_tokens(&mut self, prelude: Prelude) -> Vec<Token> {
        let html = self.atom("html");
        let head = self.atom("head");
        let body = self.atom("body");

        let mut tokens = vec![Token::StartTag {
            name: html,
            attrs: Vec::new(),
            self_closing: false,
        }];
        match prelude {
            Prelude::Head => tokens.push(Token::StartTag {
                name: head,
                attrs: Vec::new(),
                self_closing: false,
            }),
            Prelude::Body => tokens.push(Token::StartTag {
                name: body,
                attrs: Vec::new(),
                self_closing: false,
            }),
        }
        tokens
    }

    pub(super) fn process_prelude(&mut self, prelude: Prelude) {
        let tokens = self.prelude_tokens(prelude);
        self.process_all(tokens);
    }

    pub(super) fn enter_text_mode_container(
        &mut self,
        prelude: Prelude,
        tag_name: &str,
    ) -> EnteredTextMode {
        self.process_prelude(prelude);
        let tag = self.atom(tag_name);
        self.process_ok(Token::StartTag {
            name: tag,
            attrs: Vec::new(),
            self_closing: false,
        });
        EnteredTextMode { tag }
    }

    pub(super) fn state(&self) -> crate::html5::tree_builder::api::TreeBuilderStateSnapshot {
        self.builder.state_snapshot()
    }

    pub(super) fn text_patches(&mut self) -> Vec<String> {
        self.builder
            .drain_patches()
            .into_iter()
            .filter_map(|patch| match patch {
                DomPatch::CreateText { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    pub(super) fn parse_error_kinds(&mut self) -> Vec<&'static str> {
        self.builder.take_parse_error_kinds_for_test()
    }
}
