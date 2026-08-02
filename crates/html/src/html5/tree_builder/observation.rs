use crate::html5::shared::{TextValue, Token, TransitionTokenSummary};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::TreeBuilderProcessContext;
use std::sync::Arc;

#[derive(Debug, Default)]
pub(in crate::html5::tree_builder) enum LogicalTokenSummaryCache {
    #[default]
    Uninitialized,
    Ready(Arc<TransitionTokenSummary>),
    Failed,
}

#[derive(Debug)]
pub(in crate::html5::tree_builder) struct PendingTreeTransition {
    pub(in crate::html5::tree_builder) occurrence: u64,
    pub(in crate::html5::tree_builder) token: Arc<TransitionTokenSummary>,
}

pub(in crate::html5::tree_builder) fn prepare_tree_transition(
    context: &mut TreeBuilderProcessContext<'_>,
    cache: &mut LogicalTokenSummaryCache,
    token: &Token,
    text: &dyn TextResolver,
) -> Option<PendingTreeTransition> {
    let occurrence = context.reserve_tree_transition()?;
    let summary = match cache {
        LogicalTokenSummaryCache::Ready(summary) => Arc::clone(summary),
        LogicalTokenSummaryCache::Failed => return None,
        LogicalTokenSummaryCache::Uninitialized => {
            let Some(summary) = canonicalize_transition_token(token, context.atoms(), text) else {
                *cache = LogicalTokenSummaryCache::Failed;
                context.record_tree_transition_capture_failure();
                return None;
            };
            let summary = Arc::new(summary);
            *cache = LogicalTokenSummaryCache::Ready(Arc::clone(&summary));
            summary
        }
    };
    Some(PendingTreeTransition {
        occurrence,
        token: summary,
    })
}

fn canonicalize_transition_token(
    token: &Token,
    atoms: &crate::html5::shared::AtomTable,
    text: &dyn TextResolver,
) -> Option<TransitionTokenSummary> {
    Some(match token {
        Token::Doctype { .. } => TransitionTokenSummary::Doctype,
        Token::StartTag {
            name, self_closing, ..
        } => TransitionTokenSummary::StartTag {
            name: atoms.resolve(*name)?.to_string(),
            self_closing: *self_closing,
        },
        Token::EndTag { name } => TransitionTokenSummary::EndTag {
            name: atoms.resolve(*name)?.to_string(),
        },
        Token::Text { text: value } => TransitionTokenSummary::Character {
            data: resolve_text(value, text)?,
        },
        Token::Comment { .. } => TransitionTokenSummary::Comment,
        Token::ProcessingInstruction(processing_instruction) => {
            TransitionTokenSummary::ProcessingInstruction {
                target: processing_instruction.target.clone(),
            }
        }
        Token::Eof => TransitionTokenSummary::Eof,
    })
}

fn resolve_text(value: &TextValue, text: &dyn TextResolver) -> Option<String> {
    match value {
        TextValue::Owned(value) | TextValue::NullNormalized { text: value, .. } => {
            Some(value.clone())
        }
        TextValue::Span(span) => text.resolve_span(*span).ok().map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::shared::{
        DocumentParseContext, ErrorPolicy, ParserObservationCaptureFailure,
        ParserObservationConfig, ParserObservationFailure, SurfaceCaptureRequest, TextSpan,
    };
    use crate::html5::tokenizer::TextResolveError;
    use crate::html5::tree_builder::TreeBuilderProcessContext;
    use std::cell::Cell;

    struct NoSpans;

    impl TextResolver for NoSpans {
        fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
            Err(TextResolveError::InvalidSpan { span })
        }
    }

    struct CountingFailureResolver {
        calls: Cell<usize>,
    }

    impl TextResolver for CountingFailureResolver {
        fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
            self.calls.set(self.calls.get() + 1);
            Err(TextResolveError::InvalidSpan { span })
        }
    }

    fn start_tag(context: &mut DocumentParseContext) -> Token {
        let name = context
            .atoms
            .intern_ascii_folded("redispatched")
            .expect("test atom");
        Token::StartTag {
            name,
            attrs: Vec::new(),
            self_closing: false,
        }
    }

    #[test]
    fn logical_token_summary_is_shared_across_retainable_attempts() {
        let mut owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                tree_transitions: SurfaceCaptureRequest::Capture { capacity: 2 },
                ..ParserObservationConfig::default()
            },
        );
        let token = start_tag(&mut owner);
        let mut context = TreeBuilderProcessContext::new(&mut owner);
        let mut cache = LogicalTokenSummaryCache::default();
        let first = prepare_tree_transition(&mut context, &mut cache, &token, &NoSpans).unwrap();
        let second = prepare_tree_transition(&mut context, &mut cache, &token, &NoSpans).unwrap();
        assert!(Arc::ptr_eq(&first.token, &second.token));
    }

    #[test]
    fn failed_logical_token_summary_is_latched_and_never_retried() {
        let mut owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                tree_transitions: SurfaceCaptureRequest::Capture { capacity: 3 },
                ..ParserObservationConfig::default()
            },
        );
        let token = Token::Text {
            text: TextValue::Span(TextSpan::new(0, 1)),
        };
        let resolver = CountingFailureResolver {
            calls: Cell::new(0),
        };
        {
            let mut context = TreeBuilderProcessContext::new(&mut owner);
            let mut cache = LogicalTokenSummaryCache::default();

            assert!(prepare_tree_transition(&mut context, &mut cache, &token, &resolver).is_none());
            assert!(matches!(cache, LogicalTokenSummaryCache::Failed));
            assert_eq!(resolver.calls.get(), 1);

            assert!(prepare_tree_transition(&mut context, &mut cache, &token, &resolver).is_none());
            assert!(matches!(cache, LogicalTokenSummaryCache::Failed));
            assert_eq!(
                resolver.calls.get(),
                1,
                "failed summaries must not be retried"
            );

            let mut next_token_cache = LogicalTokenSummaryCache::default();
            let next = prepare_tree_transition(
                &mut context,
                &mut next_token_cache,
                &Token::Eof,
                &resolver,
            )
            .expect("a later logical token remains observable");
            assert_eq!(next.occurrence, 3);
            context.retain_tree_transition(
                next.occurrence,
                next.token,
                crate::html5::tree_builder::modes::InsertionMode::InBody,
                crate::html5::shared::TreeDispatchPath::HtmlInsertionMode(
                    crate::html5::shared::ObservedInsertionMode::InBody,
                ),
                crate::html5::tree_builder::modes::InsertionMode::InBody,
                false,
            );
        }

        let capture = owner.take_observations().unwrap();
        assert_eq!(capture.tree_transitions.items.len(), 1);
        assert_eq!(capture.tree_transitions.items[0].occurrence, 3);
        assert_eq!(capture.tree_transitions.dropped, 0);
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Capture(
                ParserObservationCaptureFailure::TreeTransitionTokenCanonicalization
            ))
        );
    }

    #[test]
    fn unrequested_zero_and_exhausted_capture_do_not_construct_a_summary() {
        let token_for = |context: &mut DocumentParseContext| start_tag(context);

        let mut unrequested_owner = DocumentParseContext::new();
        let unrequested_token = token_for(&mut unrequested_owner);
        let mut unrequested_context = TreeBuilderProcessContext::new(&mut unrequested_owner);
        let mut unrequested_cache = LogicalTokenSummaryCache::default();
        assert!(
            prepare_tree_transition(
                &mut unrequested_context,
                &mut unrequested_cache,
                &unrequested_token,
                &NoSpans,
            )
            .is_none()
        );
        assert!(matches!(
            unrequested_cache,
            LogicalTokenSummaryCache::Uninitialized
        ));

        let mut zero_owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                tree_transitions: SurfaceCaptureRequest::Capture { capacity: 0 },
                ..ParserObservationConfig::default()
            },
        );
        let zero_token = token_for(&mut zero_owner);
        let mut zero_context = TreeBuilderProcessContext::new(&mut zero_owner);
        let mut zero_cache = LogicalTokenSummaryCache::default();
        assert!(
            prepare_tree_transition(&mut zero_context, &mut zero_cache, &zero_token, &NoSpans)
                .is_none()
        );
        assert!(matches!(
            zero_cache,
            LogicalTokenSummaryCache::Uninitialized
        ));

        let mut one_owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                tree_transitions: SurfaceCaptureRequest::Capture { capacity: 1 },
                ..ParserObservationConfig::default()
            },
        );
        let one_token = token_for(&mut one_owner);
        let mut one_context = TreeBuilderProcessContext::new(&mut one_owner);
        let mut one_cache = LogicalTokenSummaryCache::default();
        let retained =
            prepare_tree_transition(&mut one_context, &mut one_cache, &one_token, &NoSpans)
                .expect("first reservation");
        one_context.retain_tree_transition(
            retained.occurrence,
            retained.token,
            crate::html5::tree_builder::modes::InsertionMode::InBody,
            crate::html5::shared::TreeDispatchPath::HtmlInsertionMode(
                crate::html5::shared::ObservedInsertionMode::InBody,
            ),
            crate::html5::tree_builder::modes::InsertionMode::InBody,
            false,
        );
        assert!(
            prepare_tree_transition(&mut one_context, &mut one_cache, &one_token, &NoSpans)
                .is_none()
        );
        assert!(matches!(one_cache, LogicalTokenSummaryCache::Ready(_)));
    }

    #[test]
    fn independent_parser_fatal_is_not_replaced_by_summary_capture_failure() {
        let mut owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                tree_transitions: SurfaceCaptureRequest::Capture { capacity: 1 },
                ..ParserObservationConfig::default()
            },
        );
        let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut owner,
        )
        .unwrap();
        let token = Token::Text {
            text: TextValue::Span(TextSpan::new(0, 1)),
        };
        assert_eq!(
            builder.process(
                &token,
                &mut TreeBuilderProcessContext::new(&mut owner),
                &NoSpans,
            ),
            Err(crate::html5::shared::ParserFatalError::EngineInvariant)
        );
        let capture = owner.take_observations().unwrap();
        assert_eq!(
            capture.failure,
            Some(crate::html5::shared::ParserObservationFailure::Capture(
                crate::html5::shared::ParserObservationCaptureFailure::
                    TreeTransitionTokenCanonicalization
            ))
        );
        assert!(capture.tree_transitions.items.is_empty());
    }
}
