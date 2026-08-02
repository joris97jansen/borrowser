use crate::html5::shared::{
    AtomTable, Attribute, TreeConstructionUnsupportedFeature, UnsupportedFeatureObservationFailure,
};
use crate::html5::tree_builder::attributes::has_missing_unqualified_token_attribute_first_wins;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderProcessContext};
use crate::names::ElementNamespace;

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn observe_repeated_html_start_rule(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        context: &mut TreeBuilderProcessContext<'_>,
    ) {
        if !context.unsupported_features_requested()
            || self
                .open_elements
                .contains_html_name(self.known_tags.template)
        {
            return;
        }
        let Some(root) = self.open_elements.get(0) else {
            return;
        };
        if root.namespace() != ElementNamespace::Html || root.name() != self.known_tags.html {
            return;
        }
        let Some((expanded_name, existing_attributes)) =
            self.live_tree.element_semantics(root.key())
        else {
            context.record_unsupported_feature_observation_failure(
                UnsupportedFeatureObservationFailure::ExistingHtmlElementSemanticsUnavailable,
            );
            return;
        };
        if expanded_name.namespace() != ElementNamespace::Html
            || expanded_name.local_name_str() != "html"
        {
            context.record_unsupported_feature_observation_failure(
                UnsupportedFeatureObservationFailure::ExistingElementIdentityContradiction,
            );
            return;
        }
        let missing = match has_missing_unqualified_token_attribute_first_wins(
            attrs,
            existing_attributes,
            atoms,
        ) {
            Ok(missing) => missing,
            Err(failure) => {
                context.record_unsupported_feature_observation_failure(failure);
                return;
            }
        };
        if missing {
            self.record_tree_unsupported_feature(
                context,
                TreeConstructionUnsupportedFeature::MergeAttributesIntoExistingHtmlElement,
            );
        }
    }

    pub(in crate::html5::tree_builder) fn observe_repeated_body_start_rule(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        context: &mut TreeBuilderProcessContext<'_>,
    ) {
        if !context.unsupported_features_requested()
            || self
                .open_elements
                .contains_html_name(self.known_tags.template)
        {
            return;
        }
        let Some(body) = self.open_elements.get(1) else {
            return;
        };
        if body.namespace() != ElementNamespace::Html || body.name() != self.known_tags.body {
            return;
        }
        if self.document_state.frameset_ok {
            self.record_tree_unsupported_feature(
                context,
                TreeConstructionUnsupportedFeature::MarkFramesetNotOkForRepeatedBodyStartTag,
            );
        }
        let Some((expanded_name, existing_attributes)) =
            self.live_tree.element_semantics(body.key())
        else {
            context.record_unsupported_feature_observation_failure(
                UnsupportedFeatureObservationFailure::ExistingBodyElementSemanticsUnavailable,
            );
            return;
        };
        if expanded_name.namespace() != ElementNamespace::Html
            || expanded_name.local_name_str() != "body"
        {
            context.record_unsupported_feature_observation_failure(
                UnsupportedFeatureObservationFailure::ExistingElementIdentityContradiction,
            );
            return;
        }

        let missing = match has_missing_unqualified_token_attribute_first_wins(
            attrs,
            existing_attributes,
            atoms,
        ) {
            Ok(missing) => missing,
            Err(failure) => {
                context.record_unsupported_feature_observation_failure(failure);
                return;
            }
        };
        if missing {
            self.record_tree_unsupported_feature(
                context,
                TreeConstructionUnsupportedFeature::MergeAttributesIntoExistingBodyElement,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::shared::{
        AttributeValue, DocumentParseContext, ErrorPolicy, ParserObservationCapture,
        ParserObservationCaptureFailure, ParserObservationConfig, ParserObservationFailure,
        SurfaceCaptureRequest, TextSpan, Token, UnsupportedFeatureEvent,
    };
    use crate::html5::tokenizer::{TextResolveError, TextResolver};
    use crate::html5::tree_builder::stack::OpenElement;
    use crate::html5::tree_builder::{TreeBuilderConfig, TreeBuilderProcessContext};

    struct NoSpans;

    impl TextResolver for NoSpans {
        fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
            Err(TextResolveError::InvalidSpan { span })
        }
    }

    fn finish_implied_document(builder: &mut Html5TreeBuilder, context: &mut DocumentParseContext) {
        let _ = builder
            .process(
                &Token::Eof,
                &mut TreeBuilderProcessContext::new(context),
                &NoSpans,
            )
            .unwrap();
    }

    fn repeated_body_builder(capacity: usize) -> (DocumentParseContext, Html5TreeBuilder) {
        let mut owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                unsupported_features: SurfaceCaptureRequest::Capture { capacity },
                ..ParserObservationConfig::default()
            },
        );
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut owner).unwrap();
        finish_implied_document(&mut builder, &mut owner);
        let _ = builder.drain_patches();
        (owner, builder)
    }

    fn missing_attribute(owner: &mut DocumentParseContext) -> Attribute {
        Attribute {
            name: owner.atoms.intern_ascii_folded("missing").unwrap(),
            value: AttributeValue::Owned("value".to_string()),
        }
    }

    fn record_repeated_body(
        builder: &mut Html5TreeBuilder,
        owner: &mut DocumentParseContext,
        attrs: &[Attribute],
    ) {
        let mut process = TreeBuilderProcessContext::new(owner);
        let atoms = process.atoms();
        builder.observe_repeated_body_start_rule(attrs, atoms, &mut process);
    }

    fn feature_occurrences(
        capture: &ParserObservationCapture,
    ) -> Vec<(u64, TreeConstructionUnsupportedFeature)> {
        capture
            .unsupported_features
            .items
            .iter()
            .map(|event| match event {
                UnsupportedFeatureEvent::TreeConstruction {
                    occurrence,
                    feature,
                    ..
                } => (*occurrence, *feature),
            })
            .collect()
    }

    #[test]
    fn unrequested_attribute_eligibility_does_not_resolve_token_names() {
        let mut owner = DocumentParseContext::new();
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut owner).unwrap();
        finish_implied_document(&mut builder, &mut owner);
        let mut foreign_atoms = crate::html5::shared::AtomTable::default();
        let invalid_here = foreign_atoms.intern_ascii_folded("missing").unwrap();
        let mut process = TreeBuilderProcessContext::new(&mut owner);
        let atoms = process.atoms();
        builder.observe_repeated_html_start_rule(
            &[Attribute {
                name: invalid_here,
                value: AttributeValue::Owned("x".to_string()),
            }],
            atoms,
            &mut process,
        );
        assert!(owner.take_observations().is_none());
    }

    #[test]
    fn requested_attribute_eligibility_failure_is_passively_latched() {
        let mut owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                unsupported_features: SurfaceCaptureRequest::Capture { capacity: 4 },
                ..ParserObservationConfig::default()
            },
        );
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut owner).unwrap();
        finish_implied_document(&mut builder, &mut owner);
        let mut foreign_atoms = crate::html5::shared::AtomTable::default();
        let invalid_here = foreign_atoms.intern_ascii_folded("missing").unwrap();
        let mut process = TreeBuilderProcessContext::new(&mut owner);
        let atoms = process.atoms();
        builder.observe_repeated_html_start_rule(
            &[Attribute {
                name: invalid_here,
                value: AttributeValue::Owned("x".to_string()),
            }],
            atoms,
            &mut process,
        );
        let capture = owner.take_observations().unwrap();
        assert!(capture.unsupported_features.items.is_empty());
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Capture(
                ParserObservationCaptureFailure::UnsupportedFeatureEligibility(
                    UnsupportedFeatureObservationFailure::TokenAttributeNameUnavailable
                )
            ))
        );
    }

    #[test]
    fn repeated_body_rule_requires_the_authoritative_second_stack_entry() {
        let mut owner = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                unsupported_features: SurfaceCaptureRequest::Capture { capacity: 4 },
                ..ParserObservationConfig::default()
            },
        );
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut owner).unwrap();
        finish_implied_document(&mut builder, &mut owner);
        let body = builder.open_elements.pop().expect("implied body");
        let div = owner.atoms.intern_ascii_folded("div").unwrap();
        builder
            .open_elements
            .try_reserve_push(ElementNamespace::Html, div)
            .unwrap();
        builder
            .open_elements
            .push(OpenElement::new_html(body.key(), div));
        let missing = owner.atoms.intern_ascii_folded("missing").unwrap();
        let mut process = TreeBuilderProcessContext::new(&mut owner);
        let atoms = process.atoms();
        builder.observe_repeated_body_start_rule(
            &[Attribute {
                name: missing,
                value: AttributeValue::Owned("x".to_string()),
            }],
            atoms,
            &mut process,
        );
        let capture = owner.take_observations().unwrap();
        assert!(capture.unsupported_features.items.is_empty());
        assert_eq!(capture.failure, None);
    }

    #[test]
    fn repeated_body_frameset_and_merge_observations_are_independent_and_ordered() {
        let frameset = TreeConstructionUnsupportedFeature::MarkFramesetNotOkForRepeatedBodyStartTag;
        let merge = TreeConstructionUnsupportedFeature::MergeAttributesIntoExistingBodyElement;

        let (mut empty_owner, mut empty_builder) = repeated_body_builder(4);
        assert!(empty_builder.document_state.frameset_ok);
        record_repeated_body(&mut empty_builder, &mut empty_owner, &[]);
        let empty_capture = empty_owner.take_observations().unwrap();
        assert_eq!(feature_occurrences(&empty_capture), vec![(1, frameset)]);
        assert!(empty_builder.document_state.frameset_ok);
        assert!(empty_builder.drain_patches().is_empty());

        let (mut merge_owner, mut merge_builder) = repeated_body_builder(4);
        merge_builder.document_state.frameset_ok = false;
        let missing = missing_attribute(&mut merge_owner);
        record_repeated_body(&mut merge_builder, &mut merge_owner, &[missing]);
        let merge_capture = merge_owner.take_observations().unwrap();
        assert_eq!(feature_occurrences(&merge_capture), vec![(1, merge)]);
        assert!(!merge_builder.document_state.frameset_ok);
        assert!(merge_builder.drain_patches().is_empty());

        let (mut both_owner, mut both_builder) = repeated_body_builder(4);
        let missing = missing_attribute(&mut both_owner);
        record_repeated_body(&mut both_builder, &mut both_owner, &[missing]);
        let both_capture = both_owner.take_observations().unwrap();
        assert_eq!(
            feature_occurrences(&both_capture),
            vec![(1, frameset), (2, merge)]
        );
        assert_eq!(both_capture.failure, None);
        assert!(both_builder.document_state.frameset_ok);
        assert!(both_builder.drain_patches().is_empty());
    }

    #[test]
    fn repeated_body_frameset_occurrence_precedes_passive_merge_eligibility_failure() {
        let (mut owner, mut builder) = repeated_body_builder(4);
        let state_before = builder.state_snapshot();
        let mut foreign_atoms = AtomTable::default();
        let unavailable_name = foreign_atoms.intern_ascii_folded("missing").unwrap();
        record_repeated_body(
            &mut builder,
            &mut owner,
            &[Attribute {
                name: unavailable_name,
                value: AttributeValue::Owned("value".to_string()),
            }],
        );

        let capture = owner.take_observations().unwrap();
        assert_eq!(
            feature_occurrences(&capture),
            vec![(
                1,
                TreeConstructionUnsupportedFeature::MarkFramesetNotOkForRepeatedBodyStartTag
            )]
        );
        assert_eq!(
            capture.failure,
            Some(ParserObservationFailure::Capture(
                ParserObservationCaptureFailure::UnsupportedFeatureEligibility(
                    UnsupportedFeatureObservationFailure::TokenAttributeNameUnavailable
                )
            ))
        );
        assert_eq!(builder.state_snapshot(), state_before);
        assert!(builder.drain_patches().is_empty());
    }

    #[test]
    fn repeated_body_applicability_suppresses_both_events_before_observer_lookup() {
        for stack_case in ["template", "missing-body", "non-body"] {
            let (mut owner, mut builder) = repeated_body_builder(4);
            match stack_case {
                "template" => {
                    let template = builder.known_tags.template;
                    builder
                        .open_elements
                        .try_reserve_push(ElementNamespace::Html, template)
                        .unwrap();
                    builder.open_elements.push(OpenElement::new_html(
                        crate::dom_patch::PatchKey(u32::MAX),
                        template,
                    ));
                }
                "missing-body" => {
                    let _ = builder.open_elements.pop().expect("implied body");
                }
                "non-body" => {
                    let body = builder.open_elements.pop().expect("implied body");
                    let div = owner.atoms.intern_ascii_folded("div").unwrap();
                    builder
                        .open_elements
                        .try_reserve_push(ElementNamespace::Html, div)
                        .unwrap();
                    builder
                        .open_elements
                        .push(OpenElement::new_html(body.key(), div));
                }
                _ => unreachable!(),
            }
            let mut foreign_atoms = AtomTable::default();
            let unavailable_name = foreign_atoms.intern_ascii_folded("missing").unwrap();
            record_repeated_body(
                &mut builder,
                &mut owner,
                &[Attribute {
                    name: unavailable_name,
                    value: AttributeValue::Owned("value".to_string()),
                }],
            );
            let capture = owner.take_observations().unwrap();
            assert!(
                capture.unsupported_features.items.is_empty(),
                "{stack_case}"
            );
            assert_eq!(capture.failure, None, "{stack_case}");
        }
    }

    #[test]
    fn repeated_body_zero_and_exhausted_capacity_drop_both_events_passively() {
        let (mut zero_owner, mut zero_builder) = repeated_body_builder(0);
        let missing = missing_attribute(&mut zero_owner);
        let zero_state = zero_builder.state_snapshot();
        record_repeated_body(&mut zero_builder, &mut zero_owner, &[missing]);
        let zero_capture = zero_owner.take_observations().unwrap();
        assert!(zero_capture.unsupported_features.items.is_empty());
        assert_eq!(zero_capture.unsupported_features.dropped, 2);
        assert_eq!(zero_capture.failure, None);
        assert_eq!(zero_builder.state_snapshot(), zero_state);
        assert!(zero_builder.drain_patches().is_empty());

        let (mut full_owner, mut full_builder) = repeated_body_builder(2);
        let missing = missing_attribute(&mut full_owner);
        record_repeated_body(
            &mut full_builder,
            &mut full_owner,
            std::slice::from_ref(&missing),
        );
        let full_state = full_builder.state_snapshot();
        record_repeated_body(&mut full_builder, &mut full_owner, &[missing]);
        let full_capture = full_owner.take_observations().unwrap();
        assert_eq!(
            feature_occurrences(&full_capture),
            vec![
                (
                    1,
                    TreeConstructionUnsupportedFeature::MarkFramesetNotOkForRepeatedBodyStartTag
                ),
                (
                    2,
                    TreeConstructionUnsupportedFeature::MergeAttributesIntoExistingBodyElement
                )
            ]
        );
        assert_eq!(full_capture.unsupported_features.dropped, 2);
        assert_eq!(full_capture.failure, None);
        assert_eq!(full_builder.state_snapshot(), full_state);
        assert!(full_builder.drain_patches().is_empty());
    }
}
