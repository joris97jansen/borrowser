//! Internal, versioned semantic observations for parser regression fixtures.
//!
//! These values are engine-test contracts. They are not DOM bindings or a
//! public web-platform API and are available only with `parser-conformance`.

use crate::html5::shared::{
    ImplementationDiagnosticEvent, ObservedToken, ParseErrorEvent, TreeTransitionEvent,
    UnsupportedFeatureEvent,
};
use crate::{AttributeNamespace, DocumentMode, ElementNamespace};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservationState<T> {
    NotRequested,
    NotApplicable {
        reason: NotApplicableReason,
    },
    Captured(T),
    Incomplete {
        partial: T,
        reason: IncompleteObservationReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotApplicableReason {
    StandaloneTokenizerRun,
    DocumentParserRun,
    FragmentParserRun,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncompleteObservationReason {
    StorageLimitExceeded { retained: usize, dropped: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalParserResult {
    pub tokens: ObservationState<Vec<ObservedToken>>,
    pub parse_errors: ObservationState<Vec<ParseErrorEvent>>,
    pub implementation_diagnostics: ObservationState<Vec<ImplementationDiagnosticEvent>>,
    pub document_mode: ObservationState<DocumentMode>,
    pub tree: ObservationState<ObservedTree>,
    pub patches: ObservationState<ObservedPatchStream>,
    pub transitions: ObservationState<Vec<TreeTransitionEvent>>,
    pub unsupported_features: ObservationState<Vec<UnsupportedFeatureEvent>>,
    pub final_invariants: ObservationState<ParserFinalizationReport>,
}

impl CanonicalParserResult {
    pub fn is_authoritative(&self) -> bool {
        observation_is_authoritative(&self.tokens)
            && observation_is_authoritative(&self.parse_errors)
            && observation_is_authoritative(&self.implementation_diagnostics)
            && observation_is_authoritative(&self.document_mode)
            && observation_is_authoritative(&self.tree)
            && observation_is_authoritative(&self.patches)
            && observation_is_authoritative(&self.transitions)
            && observation_is_authoritative(&self.unsupported_features)
            && observation_is_authoritative(&self.final_invariants)
    }

    pub fn has_failed_final_invariant(&self) -> bool {
        matches!(
            &self.final_invariants,
            ObservationState::Captured(report) if report.has_failure()
        )
    }

    pub fn failed_final_invariants(&self) -> Vec<InvariantFailureCode> {
        match &self.final_invariants {
            ObservationState::Captured(report) => report.failures(),
            ObservationState::NotRequested
            | ObservationState::NotApplicable { .. }
            | ObservationState::Incomplete { .. } => Vec::new(),
        }
    }
}

fn observation_is_authoritative<T>(observation: &ObservationState<T>) -> bool {
    !matches!(observation, ObservationState::Incomplete { .. })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservedTree {
    pub roots: Vec<ObservedTreeNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedDomAttribute {
    pub namespace: AttributeNamespace,
    pub prefix: Option<String>,
    pub local_name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedTreeNode {
    Document {
        children: Vec<ObservedTreeNode>,
    },
    DocumentType {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
    },
    Comment {
        data: String,
    },
    Text {
        data: String,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
    Element {
        namespace: ElementNamespace,
        local_name: String,
        attributes: Vec<ObservedDomAttribute>,
        children: Vec<ObservedTreeNode>,
    },
    HtmlTemplateElement {
        attributes: Vec<ObservedDomAttribute>,
        ordinary_children: Vec<ObservedTreeNode>,
        contents: ObservedTemplateContents,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservedTemplateContents {
    pub children: Vec<ObservedTreeNode>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObservedPatchStream {
    pub operations: Vec<ObservedPatchOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatchNodeLabel(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedPatchOperation {
    Clear,
    CreateDocument {
        node: PatchNodeLabel,
        legacy_doctype: Option<String>,
    },
    CreateDocumentType {
        node: PatchNodeLabel,
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
    },
    CreateElement {
        node: PatchNodeLabel,
        namespace: ElementNamespace,
        local_name: String,
        attributes: Vec<ObservedDomAttribute>,
    },
    CreateTemplateContents {
        host: PatchNodeLabel,
        contents: PatchNodeLabel,
    },
    CreateText {
        node: PatchNodeLabel,
        text: String,
    },
    CreateComment {
        node: PatchNodeLabel,
        data: String,
    },
    CreateProcessingInstruction {
        node: PatchNodeLabel,
        target: String,
        data: String,
    },
    AppendChild {
        parent: PatchNodeLabel,
        child: PatchNodeLabel,
    },
    InsertBefore {
        parent: PatchNodeLabel,
        child: PatchNodeLabel,
        before: PatchNodeLabel,
    },
    RemoveNode {
        node: PatchNodeLabel,
    },
    SetAttributes {
        node: PatchNodeLabel,
        attributes: Vec<ObservedDomAttribute>,
    },
    SetText {
        node: PatchNodeLabel,
        text: String,
    },
    AppendText {
        node: PatchNodeLabel,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserFinalizationReport {
    pub input: InputFinalizationChecks,
    pub tokenizer: TokenizerFinalizationChecks,
    pub tree_builder: TreeBuilderFinalizationChecks,
    pub dom: DomFinalizationChecks,
    pub patches: PatchFinalizationChecks,
}

impl ParserFinalizationReport {
    pub fn has_failure(&self) -> bool {
        self.failed_fields().next().is_some()
    }

    /// Fixed canonical field order without heap allocation.
    pub fn fields(&self) -> impl Iterator<Item = (FinalInvariantField, &InvariantOutcome)> {
        [
            (
                FinalInvariantField::DecoderCarryEmpty,
                &self.input.decoder_carry_empty,
            ),
            (
                FinalInvariantField::PreprocessingFlushed,
                &self.input.preprocessing_flushed,
            ),
            (
                FinalInvariantField::EofEmittedOnce,
                &self.tokenizer.eof_emitted_once,
            ),
            (
                FinalInvariantField::PendingConstructsFlushed,
                &self.tokenizer.pending_constructs_flushed,
            ),
            (
                FinalInvariantField::OutputAccountedFor,
                &self.tokenizer.output_accounted_for,
            ),
            (
                FinalInvariantField::PendingTableTextEmpty,
                &self.tree_builder.pending_table_text_empty,
            ),
            (
                FinalInvariantField::InsertionModeValid,
                &self.tree_builder.insertion_mode_valid,
            ),
            (
                FinalInvariantField::OpenElementsConsistent,
                &self.tree_builder.open_elements_consistent,
            ),
            (
                FinalInvariantField::ActiveFormattingConsistent,
                &self.tree_builder.active_formatting_consistent,
            ),
            (
                FinalInvariantField::TemplateModesConsistent,
                &self.tree_builder.template_modes_consistent,
            ),
            (
                FinalInvariantField::FormPointerValid,
                &self.tree_builder.form_pointer_valid,
            ),
            (
                FinalInvariantField::ParentChildLinksValid,
                &self.dom.parent_child_links_valid,
            ),
            (
                FinalInvariantField::NamespacesValid,
                &self.dom.namespaces_valid,
            ),
            (
                FinalInvariantField::TemplateAssociationsValid,
                &self.dom.template_associations_valid,
            ),
            (
                FinalInvariantField::AllPatchesMaterialized,
                &self.patches.all_patches_materialized,
            ),
            (
                FinalInvariantField::LiveTreeMatchesMaterializedDom,
                &self.patches.live_tree_matches_materialized_dom,
            ),
        ]
        .into_iter()
    }

    pub fn failed_fields(
        &self,
    ) -> impl Iterator<Item = (FinalInvariantField, InvariantFailureCode)> + '_ {
        self.fields().filter_map(|(field, outcome)| {
            matches!(outcome, InvariantOutcome::Failed).then_some((field, field.failure_code()))
        })
    }

    pub fn failures(&self) -> Vec<InvariantFailureCode> {
        self.failed_fields().map(|(_, code)| code).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalInvariantField {
    DecoderCarryEmpty,
    PreprocessingFlushed,
    EofEmittedOnce,
    PendingConstructsFlushed,
    OutputAccountedFor,
    PendingTableTextEmpty,
    InsertionModeValid,
    OpenElementsConsistent,
    ActiveFormattingConsistent,
    TemplateModesConsistent,
    FormPointerValid,
    ParentChildLinksValid,
    NamespacesValid,
    TemplateAssociationsValid,
    AllPatchesMaterialized,
    LiveTreeMatchesMaterializedDom,
}

impl FinalInvariantField {
    pub const fn failure_code(self) -> InvariantFailureCode {
        match self {
            Self::DecoderCarryEmpty => InvariantFailureCode::DecoderCarryNotEmpty,
            Self::PreprocessingFlushed => InvariantFailureCode::PreprocessingNotFlushed,
            Self::EofEmittedOnce => InvariantFailureCode::EofEmissionInvalid,
            Self::PendingConstructsFlushed => InvariantFailureCode::PendingTokenizerConstruct,
            Self::OutputAccountedFor => InvariantFailureCode::TokenizerOutputUnaccounted,
            Self::PendingTableTextEmpty => InvariantFailureCode::PendingTableText,
            Self::InsertionModeValid => InvariantFailureCode::InvalidInsertionMode,
            Self::OpenElementsConsistent => InvariantFailureCode::OpenElementsInconsistent,
            Self::ActiveFormattingConsistent => InvariantFailureCode::ActiveFormattingInconsistent,
            Self::TemplateModesConsistent => InvariantFailureCode::TemplateModesInconsistent,
            Self::FormPointerValid => InvariantFailureCode::FormPointerInvalid,
            Self::ParentChildLinksValid => InvariantFailureCode::ParentChildRelationshipInvalid,
            Self::NamespacesValid => InvariantFailureCode::NamespaceRelationshipInvalid,
            Self::TemplateAssociationsValid => InvariantFailureCode::TemplateAssociationInvalid,
            Self::AllPatchesMaterialized => InvariantFailureCode::PatchMaterializationIncomplete,
            Self::LiveTreeMatchesMaterializedDom => InvariantFailureCode::LiveTreeMismatch,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputFinalizationChecks {
    pub decoder_carry_empty: InvariantOutcome,
    pub preprocessing_flushed: InvariantOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenizerFinalizationChecks {
    pub eof_emitted_once: InvariantOutcome,
    pub pending_constructs_flushed: InvariantOutcome,
    pub output_accounted_for: InvariantOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeBuilderFinalizationChecks {
    pub pending_table_text_empty: InvariantOutcome,
    pub insertion_mode_valid: InvariantOutcome,
    pub open_elements_consistent: InvariantOutcome,
    pub active_formatting_consistent: InvariantOutcome,
    pub template_modes_consistent: InvariantOutcome,
    pub form_pointer_valid: InvariantOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomFinalizationChecks {
    pub parent_child_links_valid: InvariantOutcome,
    pub namespaces_valid: InvariantOutcome,
    pub template_associations_valid: InvariantOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchFinalizationChecks {
    pub all_patches_materialized: InvariantOutcome,
    pub live_tree_matches_materialized_dom: InvariantOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantOutcome {
    Satisfied,
    NotApplicable(InvariantNotApplicableReason),
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvariantNotApplicableReason {
    StandaloneTokenizerRun,
    DocumentParserRun,
    FragmentParserRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvariantFailureCode {
    DecoderCarryNotEmpty,
    PreprocessingNotFlushed,
    EofEmissionInvalid,
    PendingTokenizerConstruct,
    TokenizerOutputUnaccounted,
    PendingTableText,
    InvalidInsertionMode,
    OpenElementsInconsistent,
    ActiveFormattingInconsistent,
    TemplateModesInconsistent,
    FormPointerInvalid,
    ParentChildRelationshipInvalid,
    NamespaceRelationshipInvalid,
    TemplateAssociationInvalid,
    PatchMaterializationIncomplete,
    LiveTreeMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::shared::{
        InputCoordinateSpace, NormalizedInputPosition, NormalizedLineNumber, NormalizedScalarColumn,
    };

    #[test]
    fn normalized_line_and_scalar_column_coordinates_are_one_based() {
        assert_eq!(NormalizedLineNumber::new(0), None);
        assert_eq!(NormalizedScalarColumn::new(0), None);

        let position = NormalizedInputPosition {
            space: InputCoordinateSpace::NormalizedUtf8,
            utf8_byte_offset: 0,
            line: NormalizedLineNumber::new(1).unwrap(),
            column: NormalizedScalarColumn::new(1).unwrap(),
        };
        assert_eq!(position.utf8_byte_offset, 0);
        assert_eq!(position.line.get(), 1);
        assert_eq!(position.column.get(), 1);
    }

    #[test]
    fn finalization_fields_own_failure_identity_and_preserve_mandatory_field_order() {
        let report = ParserFinalizationReport {
            input: InputFinalizationChecks {
                decoder_carry_empty: InvariantOutcome::Failed,
                preprocessing_flushed: InvariantOutcome::Failed,
            },
            tokenizer: TokenizerFinalizationChecks {
                eof_emitted_once: InvariantOutcome::Failed,
                pending_constructs_flushed: InvariantOutcome::Failed,
                output_accounted_for: InvariantOutcome::Failed,
            },
            tree_builder: TreeBuilderFinalizationChecks {
                pending_table_text_empty: InvariantOutcome::Failed,
                insertion_mode_valid: InvariantOutcome::Failed,
                open_elements_consistent: InvariantOutcome::Failed,
                active_formatting_consistent: InvariantOutcome::Failed,
                template_modes_consistent: InvariantOutcome::Failed,
                form_pointer_valid: InvariantOutcome::Failed,
            },
            dom: DomFinalizationChecks {
                parent_child_links_valid: InvariantOutcome::Failed,
                namespaces_valid: InvariantOutcome::Failed,
                template_associations_valid: InvariantOutcome::Failed,
            },
            patches: PatchFinalizationChecks {
                all_patches_materialized: InvariantOutcome::Failed,
                live_tree_matches_materialized_dom: InvariantOutcome::Failed,
            },
        };

        assert_eq!(
            report.failures(),
            vec![
                InvariantFailureCode::DecoderCarryNotEmpty,
                InvariantFailureCode::PreprocessingNotFlushed,
                InvariantFailureCode::EofEmissionInvalid,
                InvariantFailureCode::PendingTokenizerConstruct,
                InvariantFailureCode::TokenizerOutputUnaccounted,
                InvariantFailureCode::PendingTableText,
                InvariantFailureCode::InvalidInsertionMode,
                InvariantFailureCode::OpenElementsInconsistent,
                InvariantFailureCode::ActiveFormattingInconsistent,
                InvariantFailureCode::TemplateModesInconsistent,
                InvariantFailureCode::FormPointerInvalid,
                InvariantFailureCode::ParentChildRelationshipInvalid,
                InvariantFailureCode::NamespaceRelationshipInvalid,
                InvariantFailureCode::TemplateAssociationInvalid,
                InvariantFailureCode::PatchMaterializationIncomplete,
                InvariantFailureCode::LiveTreeMismatch,
            ]
        );
    }

    #[test]
    fn failed_invariant_outcome_carries_no_cross_subsystem_identity() {
        let input_failure: InvariantOutcome = InvariantOutcome::Failed;
        assert_eq!(input_failure, InvariantOutcome::Failed);
    }

    #[test]
    fn canonical_tree_model_preserves_qualified_attributes_and_structural_template_contents() {
        let attributes = vec![
            ObservedDomAttribute {
                namespace: AttributeNamespace::Xml,
                prefix: Some("xml".to_string()),
                local_name: "lang".to_string(),
                value: "en".to_string(),
            },
            ObservedDomAttribute {
                namespace: AttributeNamespace::XLink,
                prefix: Some("xlink".to_string()),
                local_name: "href".to_string(),
                value: "#icon".to_string(),
            },
            ObservedDomAttribute {
                namespace: AttributeNamespace::Xmlns,
                prefix: None,
                local_name: "xmlns".to_string(),
                value: "http://www.w3.org/2000/svg".to_string(),
            },
            ObservedDomAttribute {
                namespace: AttributeNamespace::Xmlns,
                prefix: Some("xmlns".to_string()),
                local_name: "xlink".to_string(),
                value: "http://www.w3.org/1999/xlink".to_string(),
            },
        ];
        let tree = ObservedTree {
            roots: vec![ObservedTreeNode::Document {
                children: vec![
                    ObservedTreeNode::DocumentType {
                        name: Some("html".to_string()),
                        public_id: Some("-//W3C//DTD HTML 4.01//EN".to_string()),
                        system_id: Some("http://www.w3.org/TR/html4/strict.dtd".to_string()),
                    },
                    ObservedTreeNode::HtmlTemplateElement {
                        attributes: attributes.clone(),
                        ordinary_children: Vec::new(),
                        contents: ObservedTemplateContents {
                            children: vec![ObservedTreeNode::Element {
                                namespace: ElementNamespace::Svg,
                                local_name: "svg".to_string(),
                                attributes,
                                children: Vec::new(),
                            }],
                        },
                    },
                ],
            }],
        };

        let ObservedTreeNode::Document { children } = &tree.roots[0] else {
            panic!("root must be a document")
        };
        assert!(matches!(
            &children[0],
            ObservedTreeNode::DocumentType {
                public_id: Some(public_id),
                system_id: Some(system_id),
                ..
            } if public_id.contains("HTML 4.01") && system_id.ends_with("strict.dtd")
        ));
        let ObservedTreeNode::HtmlTemplateElement { contents, .. } = &children[1] else {
            panic!("template host must structurally own its contents")
        };
        assert!(matches!(
            &contents.children[0],
            ObservedTreeNode::Element { attributes, .. }
                if attributes[0].prefix.as_deref() == Some("xml")
                    && attributes[1].prefix.as_deref() == Some("xlink")
                    && attributes[2].namespace == AttributeNamespace::Xmlns
                    && attributes[3].prefix.as_deref() == Some("xmlns")
        ));
    }
}
