//! Internal, versioned semantic observations for parser regression fixtures.
//!
//! These values are engine-test contracts. They are not DOM bindings or a
//! public web-platform API and are available only with `parser-conformance`.

use crate::{
    AttributeNamespace, DocumentMode, DomPatch, ElementNamespace, ParserCreatedAttribute, PatchKey,
};
use std::num::NonZeroU64;

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
        !self.failed_final_invariants().is_empty()
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedTokenAttribute {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedToken {
    Doctype {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        force_quirks: bool,
    },
    StartTag {
        name: String,
        attributes: Vec<ObservedTokenAttribute>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Character {
        data: String,
    },
    Comment {
        data: String,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserStage {
    InputPreprocessing(InputPreprocessingStage),
    Tokenizer,
    TreeConstruction,
    Finalization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputPreprocessingStage {
    Utf8Decoding,
    NewlineNormalization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseErrorCode {
    Standard(WhatwgParseErrorCode),
    TreeConstruction(TreeConstructionParseErrorCode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhatwgParseErrorCode {
    UnexpectedNullCharacter,
    EofBeforeTagName,
    InvalidFirstCharacterOfTagName,
    MissingEndTagName,
    UnexpectedCharacterInAttributeName,
    DuplicateAttribute,
    UnexpectedCharacterInUnquotedAttributeValue,
    MissingAttributeValue,
    MissingWhitespaceBetweenAttributes,
    EofInComment,
    EofInDoctype,
    InvalidCharacterReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeConstructionParseErrorCode {
    UnexpectedDoctypeInBody,
    EndTagElementNotInScope,
    UnmatchedParagraphEndTag,
    NestedFormStartTag,
    NestedSelectStartTag,
    UnexpectedTokenInSelect,
    UnexpectedTokenInTable,
    UnexpectedTokenInTableBody,
    UnexpectedTokenInRow,
    UnexpectedTokenInCell,
    UnexpectedHtmlTokenInForeignContent,
    EofWithOpenTemplate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserRecoveryAction {
    IgnoreToken,
    ReprocessToken,
    InsertImpliedElement,
    GenerateImpliedEndTags,
    FosterParent,
    PopOpenElements,
    ReplaceInvalidInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseErrorEvent {
    pub occurrence: u64,
    pub stage: ParserStage,
    pub code: ParseErrorCode,
    pub recovery: Option<ParserRecoveryAction>,
    pub position: EventPosition,
    pub context: Option<ParserContextSummary>,
    pub description: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImplementationDiagnosticCode {
    InvalidUtf8Replaced,
    ParserResourceLimitActivated,
    ParserGuardrailActivated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplementationDiagnosticEvent {
    pub occurrence: u64,
    pub stage: ParserStage,
    pub code: ImplementationDiagnosticCode,
    pub position: EventPosition,
    pub context: Option<ParserContextSummary>,
    pub description: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventPosition {
    Known(InputPosition),
    Unavailable(PositionUnavailableReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputPosition {
    /// Position in the decoded and newline-normalized parser input.
    pub normalized: NormalizedInputPosition,
    /// Original byte position when, and only when, input provenance exists.
    ///
    /// AE13a does not retain a source-byte-to-normalized-input provenance map,
    /// so parser observations must use `Unavailable(NoInputProvenanceMap)`.
    pub source_bytes: SourceBytePosition,
}

/// A position in the production parser's normalized Unicode input buffer.
///
/// `utf8_byte_offset` is a zero-based byte offset into the decoded,
/// CR/LF-preprocessed UTF-8 string owned by `html5::Input`; it is never an
/// offset into the original fixture or network bytes. `line` and `column` are
/// one-based. A column counts Unicode scalar values from the beginning of the
/// current normalized line, not UTF-8 bytes, UTF-16 code units, or grapheme
/// clusters.
///
/// A non-EOF event identifies the insertion point immediately before the
/// normalized scalar that triggered the event. An EOF event identifies the
/// terminal insertion point after the last normalized scalar. A normalized LF
/// itself is on the preceding line; the next scalar begins at line + 1,
/// column 1.
///
/// CRLF and lone CR are each represented by one normalized LF before these
/// coordinates are assigned. Invalid UTF-8 replacement is likewise reflected
/// only as the resulting U+FFFD scalar, which occupies three bytes in the
/// normalized UTF-8 buffer and one scalar column. These rules make coordinates
/// independent of input delivery chunks. Recovering original byte positions
/// requires a separate provenance map, which AE13a deliberately does not add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalizedInputPosition {
    pub space: InputCoordinateSpace,
    /// Zero-based byte offset in normalized parser-input UTF-8.
    pub utf8_byte_offset: u64,
    pub line: NormalizedLineNumber,
    pub column: NormalizedScalarColumn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputCoordinateSpace {
    /// Decoded UTF-8 after CRLF and lone-CR preprocessing.
    NormalizedUtf8,
}

/// One-based line number in normalized parser input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NormalizedLineNumber(NonZeroU64);

impl NormalizedLineNumber {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// One-based Unicode-scalar column in normalized parser input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NormalizedScalarColumn(NonZeroU64);

impl NormalizedScalarColumn {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceBytePosition {
    Exact(u64),
    Unavailable(SourcePositionUnavailableReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcePositionUnavailableReason {
    NoInputProvenanceMap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionUnavailableReason {
    ParserDidNotProvidePosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserContextSummary {
    pub token_kind: Option<ParserTokenKind>,
    pub insertion_mode: Option<ObservedInsertionMode>,
    pub adjusted_current_node_namespace: Option<ElementNamespace>,
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

/// Convert one production patch without exposing its raw `PatchKey` values.
///
/// The caller owns snapshot-local label assignment. AE13a deliberately does
/// not define stream labeling or a patch-v3 serializer.
pub fn canonicalize_dom_patch(
    patch: &DomPatch,
    mut label_for: impl FnMut(PatchKey) -> PatchNodeLabel,
) -> ObservedPatchOperation {
    match patch {
        DomPatch::Clear => ObservedPatchOperation::Clear,
        DomPatch::CreateDocument { key, doctype } => ObservedPatchOperation::CreateDocument {
            node: label_for(*key),
            legacy_doctype: doctype.clone(),
        },
        DomPatch::CreateDocumentType {
            key,
            name,
            public_id,
            system_id,
        } => ObservedPatchOperation::CreateDocumentType {
            node: label_for(*key),
            name: name.clone(),
            public_id: public_id.clone(),
            system_id: system_id.clone(),
        },
        DomPatch::CreateElement {
            key,
            name,
            attributes,
        } => ObservedPatchOperation::CreateElement {
            node: label_for(*key),
            namespace: name.namespace(),
            local_name: name.local_name_str().to_string(),
            attributes: attributes.iter().map(canonicalize_dom_attribute).collect(),
        },
        DomPatch::CreateTemplateContents { host, contents } => {
            ObservedPatchOperation::CreateTemplateContents {
                host: label_for(*host),
                contents: label_for(*contents),
            }
        }
        DomPatch::CreateText { key, text } => ObservedPatchOperation::CreateText {
            node: label_for(*key),
            text: text.clone(),
        },
        DomPatch::CreateComment { key, text } => ObservedPatchOperation::CreateComment {
            node: label_for(*key),
            data: text.clone(),
        },
        DomPatch::CreateProcessingInstruction { key, target, data } => {
            ObservedPatchOperation::CreateProcessingInstruction {
                node: label_for(*key),
                target: target.clone(),
                data: data.clone(),
            }
        }
        DomPatch::AppendChild { parent, child } => ObservedPatchOperation::AppendChild {
            parent: label_for(*parent),
            child: label_for(*child),
        },
        DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => ObservedPatchOperation::InsertBefore {
            parent: label_for(*parent),
            child: label_for(*child),
            before: label_for(*before),
        },
        DomPatch::RemoveNode { key } => ObservedPatchOperation::RemoveNode {
            node: label_for(*key),
        },
        DomPatch::SetAttributes { key, attributes } => ObservedPatchOperation::SetAttributes {
            node: label_for(*key),
            attributes: attributes.iter().map(canonicalize_dom_attribute).collect(),
        },
        DomPatch::SetText { key, text } => ObservedPatchOperation::SetText {
            node: label_for(*key),
            text: text.clone(),
        },
        DomPatch::AppendText { key, text } => ObservedPatchOperation::AppendText {
            node: label_for(*key),
            text: text.clone(),
        },
    }
}

fn canonicalize_dom_attribute(attribute: &ParserCreatedAttribute) -> ObservedDomAttribute {
    ObservedDomAttribute {
        namespace: attribute.namespace(),
        prefix: attribute.prefix().map(str::to_string),
        local_name: attribute.local_name().to_string(),
        value: attribute.value().to_string(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserTokenKind {
    Doctype,
    StartTag,
    EndTag,
    Character,
    Comment,
    ProcessingInstruction,
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedInsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    AfterBody,
    AfterAfterBody,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InTemplate,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeDispatchPath {
    HtmlInsertionMode(ObservedInsertionMode),
    SharedTemplateRules,
    ForeignContent,
    TextMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionTokenSummary {
    Doctype,
    StartTag { name: String, self_closing: bool },
    EndTag { name: String },
    Character { data: String },
    Comment,
    ProcessingInstruction { target: String },
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeTransitionEvent {
    pub occurrence: u64,
    pub token: TransitionTokenSummary,
    pub insertion_mode_before: ObservedInsertionMode,
    pub dispatch_path: TreeDispatchPath,
    pub insertion_mode_after: ObservedInsertionMode,
    pub reprocessed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedFeatureEvent {
    pub occurrence: u64,
    pub classification: UnsupportedFeatureClassification,
    pub context: Option<ParserContextSummary>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedFeatureClassification {
    UnsupportedInputPreprocessingBranch,
    UnsupportedTokenizerBranch,
    UnsupportedTreeConstructionRule,
    DeferredFragmentParsing,
    DeferredScriptingDependentParsing,
    PartialForeignContentBranch,
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
        !self.failures().is_empty()
    }

    pub fn failures(&self) -> Vec<InvariantFailureCode> {
        let Self {
            input,
            tokenizer,
            tree_builder,
            dom,
            patches,
        } = self;
        let mut failures = Vec::new();
        input.append_failures(&mut failures);
        tokenizer.append_failures(&mut failures);
        tree_builder.append_failures(&mut failures);
        dom.append_failures(&mut failures);
        patches.append_failures(&mut failures);
        failures
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputFinalizationChecks {
    pub decoder_carry_empty: InvariantOutcome,
    pub preprocessing_flushed: InvariantOutcome,
}

impl InputFinalizationChecks {
    fn append_failures(&self, failures: &mut Vec<InvariantFailureCode>) {
        let Self {
            decoder_carry_empty,
            preprocessing_flushed,
        } = self;
        append_invariant_failure(
            decoder_carry_empty,
            InvariantFailureCode::DecoderCarryNotEmpty,
            failures,
        );
        append_invariant_failure(
            preprocessing_flushed,
            InvariantFailureCode::PreprocessingNotFlushed,
            failures,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenizerFinalizationChecks {
    pub eof_emitted_once: InvariantOutcome,
    pub pending_constructs_flushed: InvariantOutcome,
    pub output_accounted_for: InvariantOutcome,
}

impl TokenizerFinalizationChecks {
    fn append_failures(&self, failures: &mut Vec<InvariantFailureCode>) {
        let Self {
            eof_emitted_once,
            pending_constructs_flushed,
            output_accounted_for,
        } = self;
        append_invariant_failure(
            eof_emitted_once,
            InvariantFailureCode::EofEmissionInvalid,
            failures,
        );
        append_invariant_failure(
            pending_constructs_flushed,
            InvariantFailureCode::PendingTokenizerConstruct,
            failures,
        );
        append_invariant_failure(
            output_accounted_for,
            InvariantFailureCode::TokenizerOutputUnaccounted,
            failures,
        );
    }
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

impl TreeBuilderFinalizationChecks {
    fn append_failures(&self, failures: &mut Vec<InvariantFailureCode>) {
        let Self {
            pending_table_text_empty,
            insertion_mode_valid,
            open_elements_consistent,
            active_formatting_consistent,
            template_modes_consistent,
            form_pointer_valid,
        } = self;
        append_invariant_failure(
            pending_table_text_empty,
            InvariantFailureCode::PendingTableText,
            failures,
        );
        append_invariant_failure(
            insertion_mode_valid,
            InvariantFailureCode::InvalidInsertionMode,
            failures,
        );
        append_invariant_failure(
            open_elements_consistent,
            InvariantFailureCode::OpenElementsInconsistent,
            failures,
        );
        append_invariant_failure(
            active_formatting_consistent,
            InvariantFailureCode::ActiveFormattingInconsistent,
            failures,
        );
        append_invariant_failure(
            template_modes_consistent,
            InvariantFailureCode::TemplateModesInconsistent,
            failures,
        );
        append_invariant_failure(
            form_pointer_valid,
            InvariantFailureCode::FormPointerInvalid,
            failures,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomFinalizationChecks {
    pub parent_child_links_valid: InvariantOutcome,
    pub namespaces_valid: InvariantOutcome,
    pub template_associations_valid: InvariantOutcome,
}

impl DomFinalizationChecks {
    fn append_failures(&self, failures: &mut Vec<InvariantFailureCode>) {
        let Self {
            parent_child_links_valid,
            namespaces_valid,
            template_associations_valid,
        } = self;
        append_invariant_failure(
            parent_child_links_valid,
            InvariantFailureCode::ParentChildRelationshipInvalid,
            failures,
        );
        append_invariant_failure(
            namespaces_valid,
            InvariantFailureCode::NamespaceRelationshipInvalid,
            failures,
        );
        append_invariant_failure(
            template_associations_valid,
            InvariantFailureCode::TemplateAssociationInvalid,
            failures,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchFinalizationChecks {
    pub all_patches_materialized: InvariantOutcome,
    pub live_tree_matches_materialized_dom: InvariantOutcome,
}

impl PatchFinalizationChecks {
    fn append_failures(&self, failures: &mut Vec<InvariantFailureCode>) {
        let Self {
            all_patches_materialized,
            live_tree_matches_materialized_dom,
        } = self;
        append_invariant_failure(
            all_patches_materialized,
            InvariantFailureCode::PatchMaterializationIncomplete,
            failures,
        );
        append_invariant_failure(
            live_tree_matches_materialized_dom,
            InvariantFailureCode::LiveTreeMismatch,
            failures,
        );
    }
}

fn append_invariant_failure(
    outcome: &InvariantOutcome,
    code: InvariantFailureCode,
    failures: &mut Vec<InvariantFailureCode>,
) {
    if matches!(outcome, InvariantOutcome::Failed) {
        failures.push(code);
    }
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
    use crate::names::NameInterner;
    use crate::{ExpandedElementName, QualifiedAttributeName};

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

    #[test]
    fn canonical_patch_conversion_acknowledges_every_current_production_variant() {
        let mut names = NameInterner::new();
        let div = names.intern_exact("div").unwrap();
        let lang = names.intern_exact("lang").unwrap();
        let element_name = ExpandedElementName::new(
            ElementNamespace::Html,
            names.resolve_local_name(div).unwrap(),
        );
        let xml_lang = ParserCreatedAttribute::new(
            QualifiedAttributeName::xml(names.resolve_local_name(lang).unwrap()),
            "en".to_string(),
        );
        let patches = vec![
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: Some("html".to_string()),
            },
            DomPatch::CreateDocumentType {
                key: PatchKey(2),
                name: Some("html".to_string()),
                public_id: Some("public".to_string()),
                system_id: Some("system".to_string()),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: element_name,
                attributes: vec![xml_lang.clone()],
            },
            DomPatch::CreateTemplateContents {
                host: PatchKey(3),
                contents: PatchKey(4),
            },
            DomPatch::CreateText {
                key: PatchKey(5),
                text: "text".to_string(),
            },
            DomPatch::CreateComment {
                key: PatchKey(6),
                text: "comment".to_string(),
            },
            DomPatch::CreateProcessingInstruction {
                key: PatchKey(7),
                target: "xml".to_string(),
                data: "value".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(3),
            },
            DomPatch::InsertBefore {
                parent: PatchKey(1),
                child: PatchKey(5),
                before: PatchKey(3),
            },
            DomPatch::RemoveNode { key: PatchKey(6) },
            DomPatch::SetAttributes {
                key: PatchKey(3),
                attributes: vec![xml_lang],
            },
            DomPatch::SetText {
                key: PatchKey(5),
                text: "replacement".to_string(),
            },
            DomPatch::AppendText {
                key: PatchKey(5),
                text: " suffix".to_string(),
            },
        ];

        let observed = patches
            .iter()
            .map(|patch| {
                canonicalize_dom_patch(patch, |key| PatchNodeLabel(format!("node-{}", key.0)))
            })
            .collect::<Vec<_>>();
        assert_eq!(observed.len(), 14);
        assert!(matches!(observed[0], ObservedPatchOperation::Clear));
        assert!(matches!(
            &observed[2],
            ObservedPatchOperation::CreateDocumentType {
                public_id: Some(public_id),
                system_id: Some(system_id),
                ..
            } if public_id == "public" && system_id == "system"
        ));
        assert!(matches!(
            &observed[3],
            ObservedPatchOperation::CreateElement { attributes, .. }
                if attributes[0].namespace == AttributeNamespace::Xml
                    && attributes[0].prefix.as_deref() == Some("xml")
        ));
        assert!(matches!(
            &observed[13],
            ObservedPatchOperation::AppendText { text, .. } if text == " suffix"
        ));
    }
}
