//! Fallible canonical projection from production-owned parser results.

use super::execution::{
    ObservationReservationSite, ObservationResourceExhaustion, ParserObservationExecutionError,
    ParserObservationInvariantError,
};
use super::model::{
    IncompleteObservationReason, ObservationState, ObservedDomAttribute, ObservedPatchOperation,
    ObservedPatchStream, ObservedTemplateContents, ObservedTree, ObservedTreeNode, PatchNodeLabel,
};
use crate::html5::RawPatchHistoryCapture;
use crate::types::{DocumentFragmentNode, ParserCreatedFragmentKind};
use crate::{DomPatch, Node, ParserCreatedAttribute, PatchKey};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub(super) struct ObservationAllocationController {
    #[cfg(test)]
    selected: Option<ObservationFailureInjection>,
    #[cfg(test)]
    matching_occurrences: u64,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct ObservationFailureInjection {
    pub(super) step: ObservationAllocationStep,
    pub(super) occurrence: std::num::NonZeroU64,
}

/// Private semantic identity consumed only by the test failure selector.
///
/// Production failures continue to expose only `ObservationReservationSite`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ObservationAllocationStep {
    CanonicalTreeTraversalStack,
    CanonicalTreeFrameStack,
    CanonicalTreeChildStorage,
    CanonicalTreeString,
    CanonicalTreeAttributeStorage,
    CanonicalTreeAttributeValue,
    CanonicalPatchOperationStorage,
    CanonicalPatchPayload,
    CanonicalPatchAttributeStorage,
    CanonicalPatchAttributeValue,
    PatchCreationHistoryStorage,
    SnapshotLabelMapStorage,
    SnapshotLabelStringStorage,
}

#[cfg(test)]
impl ObservationAllocationController {
    pub(super) const fn with_failure(injection: ObservationFailureInjection) -> Self {
        Self {
            selected: Some(injection),
            matching_occurrences: 0,
        }
    }
}

impl ObservationAllocationController {
    fn before_reservation(
        &mut self,
        site: ObservationReservationSite,
        step: ObservationAllocationStep,
    ) -> Result<(), ObservationResourceExhaustion> {
        #[cfg(test)]
        if let Some(selected) = self.selected
            && selected.step == step
        {
            self.matching_occurrences += 1;
            if self.matching_occurrences == selected.occurrence.get() {
                self.selected = None;
                return Err(ObservationResourceExhaustion::at(site));
            }
        }
        let _ = (site, step);
        Ok(())
    }
}

pub(super) fn project_tree(
    document: &Node,
    capacity: usize,
    allocations: &mut ObservationAllocationController,
) -> Result<ObservationState<ObservedTree>, ParserObservationExecutionError> {
    let required = count_tree_units(document, allocations)?;
    if required > capacity {
        let dropped = u64::try_from(required).map_err(|_| tree_unit_overflow())?;
        return Ok(ObservationState::Incomplete {
            partial: ObservedTree::default(),
            reason: IncompleteObservationReason::StorageLimitExceeded {
                retained: 0,
                dropped,
            },
        });
    }

    let (tree, emitted) = build_tree(document, allocations)?;
    validate_projected_unit_count(required, emitted)?;
    Ok(ObservationState::Captured(tree))
}

fn count_tree_units(
    document: &Node,
    allocations: &mut ObservationAllocationController,
) -> Result<usize, ParserObservationExecutionError> {
    let mut walker = TreeWalker::new(document, allocations)?;
    let mut count = 0usize;
    while let Some(event) = walker.next(allocations)? {
        let additional = match event {
            TreeWalkEvent::EnterNode(_) => 1,
            TreeWalkEvent::EnterTemplateContents(_) => 1,
            TreeWalkEvent::ExitContainer => 0,
        };
        count = count
            .checked_add(additional)
            .ok_or_else(tree_unit_overflow)?;
    }
    Ok(count)
}

fn build_tree(
    document: &Node,
    allocations: &mut ObservationAllocationController,
) -> Result<(ObservedTree, usize), ParserObservationExecutionError> {
    let site = ObservationReservationSite::CanonicalTreeProjection;
    allocations.before_reservation(site, ObservationAllocationStep::CanonicalTreeFrameStack)?;
    let mut frames = Vec::new();
    frames
        .try_reserve(2)
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    let mut roots = try_node_vec(1, allocations)?;
    frames.push(TreeProjectionFrame::Root);

    let mut walker = TreeWalker::new(document, allocations)?;
    let mut emitted = 0usize;
    while let Some(event) = walker.next(allocations)? {
        match event {
            TreeWalkEvent::EnterNode(node) => {
                emitted = emitted.checked_add(1).ok_or_else(tree_unit_overflow)?;
                enter_node(node, &mut frames, &mut roots, allocations)?;
            }
            TreeWalkEvent::EnterTemplateContents(contents) => {
                emitted = emitted.checked_add(1).ok_or_else(tree_unit_overflow)?;
                if contents.kind() != ParserCreatedFragmentKind::TemplateContents
                    || !matches!(frames.last(), Some(TreeProjectionFrame::Template { .. }))
                {
                    return Err(tree_traversal_contradiction());
                }
                reserve_frame(&mut frames, allocations)?;
                frames.push(TreeProjectionFrame::TemplateContents {
                    children: try_node_vec(contents.children().len(), allocations)?,
                });
            }
            TreeWalkEvent::ExitContainer => {
                exit_frame(&mut frames, &mut roots)?;
            }
        }
    }
    if frames.len() != 1 || !matches!(frames[0], TreeProjectionFrame::Root) {
        return Err(tree_traversal_contradiction());
    }
    Ok((ObservedTree { roots }, emitted))
}

fn enter_node(
    node: &Node,
    frames: &mut Vec<TreeProjectionFrame>,
    roots: &mut Vec<ObservedTreeNode>,
    allocations: &mut ObservationAllocationController,
) -> Result<(), ParserObservationExecutionError> {
    match node {
        Node::Document { children, .. } => {
            reserve_frame(frames, allocations)?;
            frames.push(TreeProjectionFrame::Document {
                children: try_node_vec(children.len(), allocations)?,
            });
        }
        Node::DocumentType {
            name,
            public_id,
            system_id,
            ..
        } => append_projected_node(
            frames,
            roots,
            ObservedTreeNode::DocumentType {
                name: try_copy_optional_string(
                    name.as_deref(),
                    ObservationReservationSite::CanonicalTreeProjection,
                    ObservationAllocationStep::CanonicalTreeString,
                    allocations,
                )?,
                public_id: try_copy_optional_string(
                    public_id.as_deref(),
                    ObservationReservationSite::CanonicalTreeProjection,
                    ObservationAllocationStep::CanonicalTreeString,
                    allocations,
                )?,
                system_id: try_copy_optional_string(
                    system_id.as_deref(),
                    ObservationReservationSite::CanonicalTreeProjection,
                    ObservationAllocationStep::CanonicalTreeString,
                    allocations,
                )?,
            },
        )?,
        Node::Text { text, .. } => append_projected_node(
            frames,
            roots,
            ObservedTreeNode::Text {
                data: try_copy_string(
                    text,
                    ObservationReservationSite::CanonicalTreeProjection,
                    ObservationAllocationStep::CanonicalTreeString,
                    allocations,
                )?,
            },
        )?,
        Node::Comment { text, .. } => append_projected_node(
            frames,
            roots,
            ObservedTreeNode::Comment {
                data: try_copy_string(
                    text,
                    ObservationReservationSite::CanonicalTreeProjection,
                    ObservationAllocationStep::CanonicalTreeString,
                    allocations,
                )?,
            },
        )?,
        Node::ProcessingInstruction {
            processing_instruction,
        } => append_projected_node(
            frames,
            roots,
            ObservedTreeNode::ProcessingInstruction {
                target: try_copy_string(
                    processing_instruction.target(),
                    ObservationReservationSite::CanonicalTreeProjection,
                    ObservationAllocationStep::CanonicalTreeString,
                    allocations,
                )?,
                data: try_copy_string(
                    processing_instruction.data(),
                    ObservationReservationSite::CanonicalTreeProjection,
                    ObservationAllocationStep::CanonicalTreeString,
                    allocations,
                )?,
            },
        )?,
        Node::Element { element } => {
            let attributes = try_observed_attributes(
                element.attributes(),
                ObservationReservationSite::CanonicalTreeProjection,
                ObservationAllocationStep::CanonicalTreeAttributeStorage,
                ObservationAllocationStep::CanonicalTreeString,
                ObservationAllocationStep::CanonicalTreeAttributeValue,
                allocations,
            )?;
            if element.expanded_name().is_html("template") {
                if element.template_contents().is_none() {
                    return Err(tree_traversal_contradiction());
                }
                reserve_frame(frames, allocations)?;
                frames.push(TreeProjectionFrame::Template {
                    attributes,
                    ordinary_children: try_node_vec(element.children().len(), allocations)?,
                    contents: None,
                });
            } else {
                reserve_frame(frames, allocations)?;
                frames.push(TreeProjectionFrame::Element {
                    namespace: element.namespace(),
                    local_name: try_copy_string(
                        element.name(),
                        ObservationReservationSite::CanonicalTreeProjection,
                        ObservationAllocationStep::CanonicalTreeString,
                        allocations,
                    )?,
                    attributes,
                    children: try_node_vec(element.children().len(), allocations)?,
                });
            }
        }
    }
    Ok(())
}

fn exit_frame(
    frames: &mut Vec<TreeProjectionFrame>,
    roots: &mut Vec<ObservedTreeNode>,
) -> Result<(), ParserObservationExecutionError> {
    let Some(frame) = frames.pop() else {
        return Err(tree_traversal_contradiction());
    };
    let node = match frame {
        TreeProjectionFrame::Root => return Err(tree_traversal_contradiction()),
        TreeProjectionFrame::Document { children } => ObservedTreeNode::Document { children },
        TreeProjectionFrame::Element {
            namespace,
            local_name,
            attributes,
            children,
        } => ObservedTreeNode::Element {
            namespace,
            local_name,
            attributes,
            children,
        },
        TreeProjectionFrame::Template {
            attributes,
            ordinary_children,
            contents: Some(contents),
        } => ObservedTreeNode::HtmlTemplateElement {
            attributes,
            ordinary_children,
            contents,
        },
        TreeProjectionFrame::Template { contents: None, .. } => {
            return Err(tree_traversal_contradiction());
        }
        TreeProjectionFrame::TemplateContents { children } => {
            let Some(TreeProjectionFrame::Template { contents, .. }) = frames.last_mut() else {
                return Err(tree_traversal_contradiction());
            };
            if contents.is_some() {
                return Err(tree_traversal_contradiction());
            }
            *contents = Some(ObservedTemplateContents { children });
            return Ok(());
        }
    };
    append_projected_node(frames, roots, node)
}

fn append_projected_node(
    frames: &mut [TreeProjectionFrame],
    roots: &mut Vec<ObservedTreeNode>,
    node: ObservedTreeNode,
) -> Result<(), ParserObservationExecutionError> {
    match frames.last_mut() {
        Some(TreeProjectionFrame::Root) => roots.push(node),
        Some(TreeProjectionFrame::Document { children })
        | Some(TreeProjectionFrame::Element { children, .. })
        | Some(TreeProjectionFrame::TemplateContents { children }) => children.push(node),
        Some(TreeProjectionFrame::Template {
            ordinary_children, ..
        }) => ordinary_children.push(node),
        None => return Err(tree_traversal_contradiction()),
    }
    Ok(())
}

fn reserve_frame(
    frames: &mut Vec<TreeProjectionFrame>,
    allocations: &mut ObservationAllocationController,
) -> Result<(), ParserObservationExecutionError> {
    let site = ObservationReservationSite::CanonicalTreeProjection;
    allocations.before_reservation(site, ObservationAllocationStep::CanonicalTreeFrameStack)?;
    frames
        .try_reserve(1)
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    Ok(())
}

fn try_node_vec(
    capacity: usize,
    allocations: &mut ObservationAllocationController,
) -> Result<Vec<ObservedTreeNode>, ObservationResourceExhaustion> {
    let site = ObservationReservationSite::CanonicalTreeProjection;
    allocations.before_reservation(site, ObservationAllocationStep::CanonicalTreeChildStorage)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(capacity)
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    Ok(nodes)
}

enum TreeProjectionFrame {
    Root,
    Document {
        children: Vec<ObservedTreeNode>,
    },
    Element {
        namespace: crate::ElementNamespace,
        local_name: String,
        attributes: Vec<ObservedDomAttribute>,
        children: Vec<ObservedTreeNode>,
    },
    Template {
        attributes: Vec<ObservedDomAttribute>,
        ordinary_children: Vec<ObservedTreeNode>,
        contents: Option<ObservedTemplateContents>,
    },
    TemplateContents {
        children: Vec<ObservedTreeNode>,
    },
}

enum TreeWalkWork<'a> {
    EnterNode(&'a Node),
    EnterTemplateContents(&'a DocumentFragmentNode),
    ExitContainer,
}

enum TreeWalkEvent<'a> {
    EnterNode(&'a Node),
    EnterTemplateContents(&'a DocumentFragmentNode),
    ExitContainer,
}

struct TreeWalker<'a> {
    work: Vec<TreeWalkWork<'a>>,
    document_seen: bool,
}

impl<'a> TreeWalker<'a> {
    fn new(
        document: &'a Node,
        allocations: &mut ObservationAllocationController,
    ) -> Result<Self, ParserObservationExecutionError> {
        if !matches!(document, Node::Document { .. }) {
            return Err(observation_invariant(
                ParserObservationInvariantError::CanonicalTreeRootNotDocument,
            ));
        }
        let mut work = Vec::new();
        reserve_tree_work(&mut work, 1, allocations)?;
        work.push(TreeWalkWork::EnterNode(document));
        Ok(Self {
            work,
            document_seen: false,
        })
    }

    fn next(
        &mut self,
        allocations: &mut ObservationAllocationController,
    ) -> Result<Option<TreeWalkEvent<'a>>, ParserObservationExecutionError> {
        let Some(work) = self.work.pop() else {
            return Ok(None);
        };
        Ok(Some(match work {
            TreeWalkWork::ExitContainer => TreeWalkEvent::ExitContainer,
            TreeWalkWork::EnterTemplateContents(contents) => {
                if contents.kind() != ParserCreatedFragmentKind::TemplateContents {
                    return Err(observation_invariant(
                        ParserObservationInvariantError::InvalidTemplateContentsKind,
                    ));
                }
                let additional = contents
                    .children()
                    .len()
                    .checked_add(1)
                    .ok_or_else(tree_unit_overflow)?;
                reserve_tree_work(&mut self.work, additional, allocations)?;
                self.work.push(TreeWalkWork::ExitContainer);
                for child in contents.children().iter().rev() {
                    self.work.push(TreeWalkWork::EnterNode(child));
                }
                TreeWalkEvent::EnterTemplateContents(contents)
            }
            TreeWalkWork::EnterNode(node) => {
                match node {
                    Node::Document {
                        doctype, children, ..
                    } => {
                        if self.document_seen {
                            return Err(observation_invariant(
                                ParserObservationInvariantError::CanonicalTreeRootNotDocument,
                            ));
                        }
                        self.document_seen = true;
                        if doctype.is_some() {
                            return Err(observation_invariant(
                                ParserObservationInvariantError::UnexpectedLegacyDocumentDoctypeMetadata,
                            ));
                        }
                        schedule_node_children(&mut self.work, children, allocations)?;
                    }
                    Node::Element { element } => {
                        if element.expanded_name().is_html("template") {
                            let Some(contents) = element.template_contents() else {
                                return Err(observation_invariant(
                                    ParserObservationInvariantError::MissingHtmlTemplateContents,
                                ));
                            };
                            if contents.kind() != ParserCreatedFragmentKind::TemplateContents {
                                return Err(observation_invariant(
                                    ParserObservationInvariantError::InvalidTemplateContentsKind,
                                ));
                            }
                            let additional = element
                                .children()
                                .len()
                                .checked_add(2)
                                .ok_or_else(tree_unit_overflow)?;
                            reserve_tree_work(&mut self.work, additional, allocations)?;
                            self.work.push(TreeWalkWork::ExitContainer);
                            self.work
                                .push(TreeWalkWork::EnterTemplateContents(contents));
                            for child in element.children().iter().rev() {
                                self.work.push(TreeWalkWork::EnterNode(child));
                            }
                        } else {
                            schedule_node_children(
                                &mut self.work,
                                element.children(),
                                allocations,
                            )?;
                        }
                    }
                    Node::DocumentType { .. }
                    | Node::Text { .. }
                    | Node::Comment { .. }
                    | Node::ProcessingInstruction { .. } => {}
                }
                TreeWalkEvent::EnterNode(node)
            }
        }))
    }
}

fn schedule_node_children<'a>(
    work: &mut Vec<TreeWalkWork<'a>>,
    children: &'a [Node],
    allocations: &mut ObservationAllocationController,
) -> Result<(), ParserObservationExecutionError> {
    let additional = children
        .len()
        .checked_add(1)
        .ok_or_else(tree_unit_overflow)?;
    reserve_tree_work(work, additional, allocations)?;
    work.push(TreeWalkWork::ExitContainer);
    for child in children.iter().rev() {
        work.push(TreeWalkWork::EnterNode(child));
    }
    Ok(())
}

fn reserve_tree_work<'a>(
    work: &mut Vec<TreeWalkWork<'a>>,
    additional: usize,
    allocations: &mut ObservationAllocationController,
) -> Result<(), ParserObservationExecutionError> {
    let site = ObservationReservationSite::CanonicalTreeProjection;
    allocations.before_reservation(site, ObservationAllocationStep::CanonicalTreeTraversalStack)?;
    work.try_reserve(additional)
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    Ok(())
}

pub(super) fn project_patches(
    capture: RawPatchHistoryCapture,
    allocations: &mut ObservationAllocationController,
) -> Result<ObservationState<ObservedPatchStream>, ParserObservationExecutionError> {
    let site = ObservationReservationSite::CanonicalPatchProjection;
    allocations.before_reservation(
        site,
        ObservationAllocationStep::CanonicalPatchOperationStorage,
    )?;
    let mut operations = Vec::new();
    operations
        .try_reserve_exact(capture.operations.len())
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    let mut labels = SnapshotLabels::default();
    let mut created = HashSet::new();
    for patch in &capture.operations {
        validate_patch_history(patch, &mut created, allocations)?;
        operations.push(canonicalize_patch(patch, &mut labels, allocations)?);
    }
    let stream = ObservedPatchStream { operations };
    if capture.dropped == 0 {
        Ok(ObservationState::Captured(stream))
    } else {
        Ok(ObservationState::Incomplete {
            partial: stream,
            reason: IncompleteObservationReason::StorageLimitExceeded {
                retained: capture.operations.len(),
                dropped: capture.dropped,
            },
        })
    }
}

fn validate_patch_history(
    patch: &DomPatch,
    created: &mut HashSet<PatchKey>,
    allocations: &mut ObservationAllocationController,
) -> Result<(), ParserObservationExecutionError> {
    match patch {
        DomPatch::Clear => Ok(()),
        DomPatch::CreateDocument { key, .. }
        | DomPatch::CreateDocumentType { key, .. }
        | DomPatch::CreateElement { key, .. }
        | DomPatch::CreateText { key, .. }
        | DomPatch::CreateComment { key, .. }
        | DomPatch::CreateProcessingInstruction { key, .. } => {
            introduce_key(*key, created, allocations)
        }
        DomPatch::CreateTemplateContents { host, contents } => {
            require_key(*host, created)?;
            introduce_key(*contents, created, allocations)
        }
        DomPatch::AppendChild { parent, child } => {
            require_key(*parent, created)?;
            require_key(*child, created)
        }
        DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => {
            require_key(*parent, created)?;
            require_key(*child, created)?;
            require_key(*before, created)
        }
        DomPatch::RemoveNode { key }
        | DomPatch::SetAttributes { key, .. }
        | DomPatch::SetText { key, .. }
        | DomPatch::AppendText { key, .. } => require_key(*key, created),
    }
}

fn introduce_key(
    key: PatchKey,
    created: &mut HashSet<PatchKey>,
    allocations: &mut ObservationAllocationController,
) -> Result<(), ParserObservationExecutionError> {
    if key == PatchKey::INVALID {
        return Err(observation_invariant(
            ParserObservationInvariantError::InvalidPatchKey,
        ));
    }
    if created.contains(&key) {
        return Err(observation_invariant(
            ParserObservationInvariantError::DuplicatePatchCreation,
        ));
    }
    let site = ObservationReservationSite::CanonicalPatchProjection;
    allocations.before_reservation(site, ObservationAllocationStep::PatchCreationHistoryStorage)?;
    created
        .try_reserve(1)
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    created.insert(key);
    Ok(())
}

fn require_key(
    key: PatchKey,
    created: &HashSet<PatchKey>,
) -> Result<(), ParserObservationExecutionError> {
    if key == PatchKey::INVALID {
        return Err(observation_invariant(
            ParserObservationInvariantError::InvalidPatchKey,
        ));
    }
    if !created.contains(&key) {
        return Err(observation_invariant(
            ParserObservationInvariantError::MissingPatchCreationHistory,
        ));
    }
    Ok(())
}

#[derive(Default)]
struct SnapshotLabels {
    by_key: HashMap<PatchKey, u64>,
    next: u64,
}

impl SnapshotLabels {
    fn label(
        &mut self,
        key: PatchKey,
        allocations: &mut ObservationAllocationController,
    ) -> Result<PatchNodeLabel, ParserObservationExecutionError> {
        if key == PatchKey::INVALID {
            return Err(observation_invariant(
                ParserObservationInvariantError::InvalidPatchKey,
            ));
        }
        let number = if let Some(number) = self.by_key.get(&key) {
            *number
        } else {
            let number = self.next.checked_add(1).ok_or_else(|| {
                observation_invariant(
                    ParserObservationInvariantError::SnapshotLabelSequenceOverflow,
                )
            })?;
            let site = ObservationReservationSite::SnapshotLabelStorage;
            allocations
                .before_reservation(site, ObservationAllocationStep::SnapshotLabelMapStorage)?;
            self.by_key
                .try_reserve(1)
                .map_err(|_| ObservationResourceExhaustion::at(site))?;
            self.by_key.insert(key, number);
            self.next = number;
            number
        };
        Ok(PatchNodeLabel(try_label_string(number, allocations)?))
    }
}

fn try_label_string(
    number: u64,
    allocations: &mut ObservationAllocationController,
) -> Result<String, ParserObservationExecutionError> {
    let mut digits = [0u8; 20];
    let mut cursor = digits.len();
    let mut remaining = number;
    loop {
        cursor -= 1;
        digits[cursor] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    let digit_text = std::str::from_utf8(&digits[cursor..]).map_err(|_| {
        observation_invariant(ParserObservationInvariantError::SnapshotLabelSequenceOverflow)
    })?;
    let site = ObservationReservationSite::SnapshotLabelStorage;
    allocations.before_reservation(site, ObservationAllocationStep::SnapshotLabelStringStorage)?;
    let capacity = 5usize
        .checked_add(digit_text.len())
        .ok_or_else(|| ObservationResourceExhaustion::at(site))?;
    let mut label = String::new();
    label
        .try_reserve_exact(capacity)
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    label.push_str("node-");
    label.push_str(digit_text);
    Ok(label)
}

fn canonicalize_patch(
    patch: &DomPatch,
    labels: &mut SnapshotLabels,
    allocations: &mut ObservationAllocationController,
) -> Result<ObservedPatchOperation, ParserObservationExecutionError> {
    let site = ObservationReservationSite::CanonicalPatchProjection;
    Ok(match patch {
        DomPatch::Clear => ObservedPatchOperation::Clear,
        DomPatch::CreateDocument { key, doctype } => ObservedPatchOperation::CreateDocument {
            node: labels.label(*key, allocations)?,
            legacy_doctype: try_copy_optional_string(
                doctype.as_deref(),
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
        },
        DomPatch::CreateDocumentType {
            key,
            name,
            public_id,
            system_id,
        } => ObservedPatchOperation::CreateDocumentType {
            node: labels.label(*key, allocations)?,
            name: try_copy_optional_string(
                name.as_deref(),
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
            public_id: try_copy_optional_string(
                public_id.as_deref(),
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
            system_id: try_copy_optional_string(
                system_id.as_deref(),
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
        },
        DomPatch::CreateElement {
            key,
            name,
            attributes,
        } => ObservedPatchOperation::CreateElement {
            node: labels.label(*key, allocations)?,
            namespace: name.namespace(),
            local_name: try_copy_string(
                name.local_name_str(),
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
            attributes: try_observed_attributes(
                attributes,
                site,
                ObservationAllocationStep::CanonicalPatchAttributeStorage,
                ObservationAllocationStep::CanonicalPatchPayload,
                ObservationAllocationStep::CanonicalPatchAttributeValue,
                allocations,
            )?,
        },
        DomPatch::CreateTemplateContents { host, contents } => {
            ObservedPatchOperation::CreateTemplateContents {
                host: labels.label(*host, allocations)?,
                contents: labels.label(*contents, allocations)?,
            }
        }
        DomPatch::CreateText { key, text } => ObservedPatchOperation::CreateText {
            node: labels.label(*key, allocations)?,
            text: try_copy_string(
                text,
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
        },
        DomPatch::CreateComment { key, text } => ObservedPatchOperation::CreateComment {
            node: labels.label(*key, allocations)?,
            data: try_copy_string(
                text,
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
        },
        DomPatch::CreateProcessingInstruction { key, target, data } => {
            ObservedPatchOperation::CreateProcessingInstruction {
                node: labels.label(*key, allocations)?,
                target: try_copy_string(
                    target,
                    site,
                    ObservationAllocationStep::CanonicalPatchPayload,
                    allocations,
                )?,
                data: try_copy_string(
                    data,
                    site,
                    ObservationAllocationStep::CanonicalPatchPayload,
                    allocations,
                )?,
            }
        }
        DomPatch::AppendChild { parent, child } => ObservedPatchOperation::AppendChild {
            parent: labels.label(*parent, allocations)?,
            child: labels.label(*child, allocations)?,
        },
        DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => ObservedPatchOperation::InsertBefore {
            parent: labels.label(*parent, allocations)?,
            child: labels.label(*child, allocations)?,
            before: labels.label(*before, allocations)?,
        },
        DomPatch::RemoveNode { key } => ObservedPatchOperation::RemoveNode {
            node: labels.label(*key, allocations)?,
        },
        DomPatch::SetAttributes { key, attributes } => ObservedPatchOperation::SetAttributes {
            node: labels.label(*key, allocations)?,
            attributes: try_observed_attributes(
                attributes,
                site,
                ObservationAllocationStep::CanonicalPatchAttributeStorage,
                ObservationAllocationStep::CanonicalPatchPayload,
                ObservationAllocationStep::CanonicalPatchAttributeValue,
                allocations,
            )?,
        },
        DomPatch::SetText { key, text } => ObservedPatchOperation::SetText {
            node: labels.label(*key, allocations)?,
            text: try_copy_string(
                text,
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
        },
        DomPatch::AppendText { key, text } => ObservedPatchOperation::AppendText {
            node: labels.label(*key, allocations)?,
            text: try_copy_string(
                text,
                site,
                ObservationAllocationStep::CanonicalPatchPayload,
                allocations,
            )?,
        },
    })
}

fn try_observed_attributes(
    attributes: &[ParserCreatedAttribute],
    site: ObservationReservationSite,
    storage_step: ObservationAllocationStep,
    string_step: ObservationAllocationStep,
    value_step: ObservationAllocationStep,
    allocations: &mut ObservationAllocationController,
) -> Result<Vec<ObservedDomAttribute>, ObservationResourceExhaustion> {
    allocations.before_reservation(site, storage_step)?;
    let mut observed = Vec::new();
    observed
        .try_reserve_exact(attributes.len())
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    for attribute in attributes {
        observed.push(ObservedDomAttribute {
            namespace: attribute.namespace(),
            prefix: try_copy_optional_string(attribute.prefix(), site, string_step, allocations)?,
            local_name: try_copy_string(attribute.local_name(), site, string_step, allocations)?,
            value: try_copy_string(attribute.value(), site, value_step, allocations)?,
        });
    }
    Ok(observed)
}

fn try_copy_optional_string(
    value: Option<&str>,
    site: ObservationReservationSite,
    step: ObservationAllocationStep,
    allocations: &mut ObservationAllocationController,
) -> Result<Option<String>, ObservationResourceExhaustion> {
    value
        .map(|value| try_copy_string(value, site, step, allocations))
        .transpose()
}

fn try_copy_string(
    value: &str,
    site: ObservationReservationSite,
    step: ObservationAllocationStep,
    allocations: &mut ObservationAllocationController,
) -> Result<String, ObservationResourceExhaustion> {
    allocations.before_reservation(site, step)?;
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| ObservationResourceExhaustion::at(site))?;
    copy.push_str(value);
    Ok(copy)
}

fn tree_unit_overflow() -> ParserObservationExecutionError {
    observation_invariant(ParserObservationInvariantError::CanonicalTreeUnitCountOverflow)
}

fn validate_projected_unit_count(
    preflight: usize,
    projected: usize,
) -> Result<(), ParserObservationExecutionError> {
    if preflight == projected {
        Ok(())
    } else {
        Err(observation_invariant(
            ParserObservationInvariantError::CanonicalTreePreflightProjectionMismatch,
        ))
    }
}

fn tree_traversal_contradiction() -> ParserObservationExecutionError {
    observation_invariant(ParserObservationInvariantError::CanonicalTreeTraversalContradiction)
}

fn observation_invariant(
    invariant: ParserObservationInvariantError,
) -> ParserObservationExecutionError {
    ParserObservationExecutionError::ObservationInvariant(invariant)
}

impl From<ObservationResourceExhaustion> for ParserObservationExecutionError {
    fn from(error: ObservationResourceExhaustion) -> Self {
        Self::ResourceExhaustion(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::QualifiedAttributeName;
    use crate::names::NameInterner;
    use crate::types::{DocumentFragmentNode, Id};
    use crate::{ElementNamespace, ExpandedElementName, ParserCreatedAttribute};
    use std::num::NonZeroU64;

    fn injection(
        step: ObservationAllocationStep,
        occurrence: u64,
    ) -> ObservationAllocationController {
        ObservationAllocationController::with_failure(ObservationFailureInjection {
            step,
            occurrence: NonZeroU64::new(occurrence).expect("non-zero occurrence"),
        })
    }

    fn raw(operations: Vec<DomPatch>, dropped: u64) -> RawPatchHistoryCapture {
        let capacity = operations.len();
        RawPatchHistoryCapture {
            operations,
            dropped,
            capacity,
        }
    }

    #[test]
    fn patch_projection_covers_every_variant_and_fixed_operand_order() {
        let mut names = NameInterner::new();
        let div = names.intern_exact("div").unwrap();
        let lang = names.intern_exact("lang").unwrap();
        let element_name = ExpandedElementName::new(
            ElementNamespace::Html,
            names.resolve_local_name(div).unwrap(),
        );
        let xml_lang = ParserCreatedAttribute::new(
            QualifiedAttributeName::xml(names.resolve_local_name(lang).unwrap()),
            "en".to_owned(),
        );
        let patches = vec![
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(91),
                doctype: Some("legacy".to_owned()),
            },
            DomPatch::CreateDocumentType {
                key: PatchKey(8),
                name: Some("html".to_owned()),
                public_id: Some("public".to_owned()),
                system_id: Some("system".to_owned()),
            },
            DomPatch::CreateElement {
                key: PatchKey(44),
                name: element_name,
                attributes: vec![xml_lang.clone()],
            },
            DomPatch::CreateTemplateContents {
                host: PatchKey(44),
                contents: PatchKey(3),
            },
            DomPatch::CreateText {
                key: PatchKey(77),
                text: "text".to_owned(),
            },
            DomPatch::CreateComment {
                key: PatchKey(12),
                text: "comment".to_owned(),
            },
            DomPatch::CreateProcessingInstruction {
                key: PatchKey(66),
                target: "xml".to_owned(),
                data: "value".to_owned(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(91),
                child: PatchKey(8),
            },
            DomPatch::InsertBefore {
                parent: PatchKey(91),
                child: PatchKey(77),
                before: PatchKey(8),
            },
            DomPatch::RemoveNode { key: PatchKey(12) },
            DomPatch::SetAttributes {
                key: PatchKey(44),
                attributes: vec![xml_lang],
            },
            DomPatch::SetText {
                key: PatchKey(77),
                text: "replacement".to_owned(),
            },
            DomPatch::AppendText {
                key: PatchKey(77),
                text: " suffix".to_owned(),
            },
        ];
        let state = project_patches(
            raw(patches, 0),
            &mut ObservationAllocationController::default(),
        )
        .unwrap();
        let ObservationState::Captured(stream) = state else {
            panic!("expected complete patch stream");
        };
        assert_eq!(stream.operations.len(), 14);
        assert!(matches!(
            stream.operations[0],
            ObservedPatchOperation::Clear
        ));
        assert!(matches!(
            &stream.operations[1],
            ObservedPatchOperation::CreateDocument { node, .. } if node.0 == "node-1"
        ));
        assert!(matches!(
            &stream.operations[4],
            ObservedPatchOperation::CreateTemplateContents { host, contents }
                if host.0 == "node-3" && contents.0 == "node-4"
        ));
        assert!(matches!(
            &stream.operations[9],
            ObservedPatchOperation::InsertBefore {
                parent,
                child,
                before,
            } if parent.0 == "node-1" && child.0 == "node-5" && before.0 == "node-2"
        ));
        assert!(matches!(
            &stream.operations[11],
            ObservedPatchOperation::SetAttributes { attributes, .. }
                if attributes[0].namespace == crate::AttributeNamespace::Xml
                    && attributes[0].prefix.as_deref() == Some("xml")
        ));
    }

    #[test]
    fn clear_preserves_label_sequence_and_historical_key_reuse_is_rejected() {
        let state = project_patches(
            raw(
                vec![
                    DomPatch::CreateDocument {
                        key: PatchKey(1),
                        doctype: None,
                    },
                    DomPatch::Clear,
                    DomPatch::CreateDocument {
                        key: PatchKey(2),
                        doctype: None,
                    },
                ],
                0,
            ),
            &mut ObservationAllocationController::default(),
        )
        .unwrap();
        let ObservationState::Captured(stream) = state else {
            panic!("captured");
        };
        assert!(matches!(
            &stream.operations[2],
            ObservedPatchOperation::CreateDocument { node, .. } if node.0 == "node-2"
        ));

        assert_eq!(
            project_patches(
                raw(
                    vec![
                        DomPatch::CreateDocument {
                            key: PatchKey(1),
                            doctype: None,
                        },
                        DomPatch::Clear,
                        DomPatch::CreateText {
                            key: PatchKey(1),
                            text: "reuse".to_owned(),
                        },
                    ],
                    0,
                ),
                &mut ObservationAllocationController::default(),
            ),
            Err(observation_invariant(
                ParserObservationInvariantError::DuplicatePatchCreation
            ))
        );
    }

    #[test]
    fn retained_prefix_rejects_invalid_or_missing_creation_history() {
        for (patch, expected) in [
            (
                DomPatch::CreateText {
                    key: PatchKey::INVALID,
                    text: "x".to_owned(),
                },
                ParserObservationInvariantError::InvalidPatchKey,
            ),
            (
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                ParserObservationInvariantError::MissingPatchCreationHistory,
            ),
        ] {
            assert_eq!(
                project_patches(
                    raw(vec![patch], 0),
                    &mut ObservationAllocationController::default(),
                ),
                Err(observation_invariant(expected))
            );
        }
    }

    #[test]
    fn patch_overflow_keeps_exact_prefix_and_dropped_count() {
        let state = project_patches(
            raw(
                vec![DomPatch::CreateDocument {
                    key: PatchKey(9),
                    doctype: None,
                }],
                3,
            ),
            &mut ObservationAllocationController::default(),
        )
        .unwrap();
        let ObservationState::Incomplete { partial, reason } = state else {
            panic!("incomplete");
        };
        assert_eq!(partial.operations.len(), 1);
        assert_eq!(
            reason,
            IncompleteObservationReason::StorageLimitExceeded {
                retained: 1,
                dropped: 3,
            }
        );
    }

    #[test]
    fn canonical_label_storage_failures_are_typed_and_atomic() {
        let patch = || {
            raw(
                vec![DomPatch::CreateDocument {
                    key: PatchKey(7),
                    doctype: None,
                }],
                0,
            )
        };
        for step in [
            ObservationAllocationStep::SnapshotLabelMapStorage,
            ObservationAllocationStep::SnapshotLabelStringStorage,
        ] {
            let error = project_patches(patch(), &mut injection(step, 1)).unwrap_err();
            assert_eq!(
                error,
                ParserObservationExecutionError::ResourceExhaustion(
                    ObservationResourceExhaustion::at(
                        ObservationReservationSite::SnapshotLabelStorage
                    )
                )
            );
        }
    }

    #[test]
    fn snapshot_label_sequence_overflow_is_a_typed_invariant() {
        let mut labels = SnapshotLabels {
            by_key: HashMap::new(),
            next: u64::MAX,
        };
        assert_eq!(
            labels.label(PatchKey(1), &mut ObservationAllocationController::default()),
            Err(observation_invariant(
                ParserObservationInvariantError::SnapshotLabelSequenceOverflow
            ))
        );
    }

    #[test]
    fn canonical_patch_nested_payload_failures_are_typed_and_atomic() {
        let text = || {
            raw(
                vec![DomPatch::CreateText {
                    key: PatchKey(1),
                    text: "text".to_owned(),
                }],
                0,
            )
        };
        let doctype = || {
            raw(
                vec![DomPatch::CreateDocumentType {
                    key: PatchKey(1),
                    name: Some("html".to_owned()),
                    public_id: Some("public".to_owned()),
                    system_id: Some("system".to_owned()),
                }],
                0,
            )
        };
        let processing_instruction = || {
            raw(
                vec![DomPatch::CreateProcessingInstruction {
                    key: PatchKey(1),
                    target: "target".to_owned(),
                    data: "data".to_owned(),
                }],
                0,
            )
        };
        for (capture, step, occurrence) in [
            (text(), ObservationAllocationStep::CanonicalPatchPayload, 1),
            (
                doctype(),
                ObservationAllocationStep::CanonicalPatchPayload,
                1,
            ),
            (
                doctype(),
                ObservationAllocationStep::CanonicalPatchPayload,
                2,
            ),
            (
                doctype(),
                ObservationAllocationStep::CanonicalPatchPayload,
                3,
            ),
            (
                processing_instruction(),
                ObservationAllocationStep::CanonicalPatchPayload,
                1,
            ),
            (
                processing_instruction(),
                ObservationAllocationStep::CanonicalPatchPayload,
                2,
            ),
        ] {
            assert_eq!(
                project_patches(capture, &mut injection(step, occurrence)),
                Err(ParserObservationExecutionError::ResourceExhaustion(
                    ObservationResourceExhaustion::at(
                        ObservationReservationSite::CanonicalPatchProjection
                    )
                ))
            );
        }

        let mut names = NameInterner::new();
        let div = names.intern_exact("div").unwrap();
        let lang = names.intern_exact("lang").unwrap();
        for step in [
            ObservationAllocationStep::CanonicalPatchAttributeStorage,
            ObservationAllocationStep::CanonicalPatchAttributeValue,
        ] {
            let capture = raw(
                vec![DomPatch::CreateElement {
                    key: PatchKey(1),
                    name: ExpandedElementName::new(
                        ElementNamespace::Html,
                        names.resolve_local_name(div).unwrap(),
                    ),
                    attributes: vec![ParserCreatedAttribute::new(
                        QualifiedAttributeName::xml(names.resolve_local_name(lang).unwrap()),
                        "en".to_owned(),
                    )],
                }],
                0,
            );
            assert!(matches!(
                project_patches(capture, &mut injection(step, 1)),
                Err(ParserObservationExecutionError::ResourceExhaustion(exhaustion))
                    if exhaustion.site()
                        == ObservationReservationSite::CanonicalPatchProjection
            ));
        }

        for step in [
            ObservationAllocationStep::CanonicalPatchOperationStorage,
            ObservationAllocationStep::PatchCreationHistoryStorage,
        ] {
            assert!(matches!(
                project_patches(text(), &mut injection(step, 1)),
                Err(ParserObservationExecutionError::ResourceExhaustion(exhaustion))
                    if exhaustion.site()
                        == ObservationReservationSite::CanonicalPatchProjection
            ));
        }
    }

    fn template_tree() -> Node {
        let mut names = NameInterner::new();
        let template = names.intern_exact("template").unwrap();
        Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::from_element_parts(
                Id(2),
                ExpandedElementName::new(
                    ElementNamespace::Html,
                    names.resolve_local_name(template).unwrap(),
                ),
                Vec::new(),
                Vec::new(),
                Some(Box::new(DocumentFragmentNode::new_template_contents(
                    Id(3),
                    vec![Node::Comment {
                        id: Id(5),
                        text: "contents".to_owned(),
                    }],
                ))),
                vec![Node::Text {
                    id: Id(4),
                    text: "ordinary".to_owned(),
                }],
            )],
        }
    }

    #[test]
    fn template_walk_visits_ordinary_children_before_typed_contents() {
        let document = template_tree();
        let state = project_tree(
            &document,
            usize::MAX,
            &mut ObservationAllocationController::default(),
        )
        .unwrap();
        let ObservationState::Captured(tree) = state else {
            panic!("captured");
        };
        let ObservedTreeNode::Document { children } = &tree.roots[0] else {
            panic!("document");
        };
        let ObservedTreeNode::HtmlTemplateElement {
            ordinary_children,
            contents,
            ..
        } = &children[0]
        else {
            panic!("template");
        };
        assert!(matches!(
            &ordinary_children[0],
            ObservedTreeNode::Text { data } if data == "ordinary"
        ));
        assert!(matches!(
            &contents.children[0],
            ObservedTreeNode::Comment { data } if data == "contents"
        ));

        // The shared walker projects the ordinary text payload before the
        // template-contents comment payload. Selecting within the semantic
        // string step proves that unrelated stack reservations cannot retarget
        // either failure.
        for occurrence in [1, 2] {
            assert_eq!(
                project_tree(
                    &document,
                    usize::MAX,
                    &mut injection(ObservationAllocationStep::CanonicalTreeString, occurrence),
                ),
                Err(ParserObservationExecutionError::ResourceExhaustion(
                    ObservationResourceExhaustion::at(
                        ObservationReservationSite::CanonicalTreeProjection
                    )
                ))
            );
        }
    }

    #[test]
    fn canonical_tree_leaf_payload_failures_are_typed_and_atomic() {
        let leaves = [
            (
                Node::Text {
                    id: Id(2),
                    text: "text".to_owned(),
                },
                vec![1],
            ),
            (
                Node::Comment {
                    id: Id(2),
                    text: "comment".to_owned(),
                },
                vec![1],
            ),
            (
                Node::DocumentType {
                    id: Id(2),
                    name: Some("html".to_owned()),
                    public_id: Some("public".to_owned()),
                    system_id: Some("system".to_owned()),
                },
                vec![1, 2, 3],
            ),
            (
                Node::ProcessingInstruction {
                    processing_instruction:
                        crate::ProcessingInstructionNode::try_from_parser_created_parts(
                            Id(2),
                            "target".to_owned(),
                            "data".to_owned(),
                        )
                        .unwrap(),
                },
                vec![1, 2],
            ),
        ];
        for (leaf, occurrences) in leaves {
            let document = Node::Document {
                id: Id(1),
                doctype: None,
                children: vec![leaf],
            };
            for occurrence in occurrences {
                assert!(matches!(
                    project_tree(
                        &document,
                        usize::MAX,
                        &mut injection(
                            ObservationAllocationStep::CanonicalTreeString,
                            occurrence,
                        ),
                    ),
                    Err(ParserObservationExecutionError::ResourceExhaustion(exhaustion))
                        if exhaustion.site()
                            == ObservationReservationSite::CanonicalTreeProjection
                ));
            }
        }
    }

    #[test]
    fn canonical_tree_attribute_vector_and_value_failures_are_typed() {
        let mut names = NameInterner::new();
        let div = names.intern_exact("div").unwrap();
        let lang = names.intern_exact("lang").unwrap();
        let document = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::from_element_parts(
                Id(2),
                ExpandedElementName::new(
                    ElementNamespace::Html,
                    names.resolve_local_name(div).unwrap(),
                ),
                vec![ParserCreatedAttribute::new(
                    QualifiedAttributeName::xml(names.resolve_local_name(lang).unwrap()),
                    "en".to_owned(),
                )],
                Vec::new(),
                None,
                Vec::new(),
            )],
        };
        for step in [
            ObservationAllocationStep::CanonicalTreeAttributeStorage,
            ObservationAllocationStep::CanonicalTreeAttributeValue,
        ] {
            assert!(matches!(
                project_tree(
                    &document,
                    usize::MAX,
                    &mut injection(step, 1),
                ),
                Err(ParserObservationExecutionError::ResourceExhaustion(exhaustion))
                    if exhaustion.site()
                        == ObservationReservationSite::CanonicalTreeProjection
            ));
        }
    }

    #[test]
    fn canonical_tree_container_allocation_steps_are_semantically_targeted() {
        let document = Node::Document {
            id: Id(1),
            doctype: None,
            children: Vec::new(),
        };
        for step in [
            ObservationAllocationStep::CanonicalTreeTraversalStack,
            ObservationAllocationStep::CanonicalTreeFrameStack,
            ObservationAllocationStep::CanonicalTreeChildStorage,
        ] {
            assert!(matches!(
                project_tree(&document, 1, &mut injection(step, 1)),
                Err(ParserObservationExecutionError::ResourceExhaustion(exhaustion))
                    if exhaustion.site()
                        == ObservationReservationSite::CanonicalTreeProjection
            ));
        }
    }

    #[test]
    fn tree_capacity_is_atomic_in_structural_units() {
        let document = template_tree();
        let exact = 5; // document, template, ordinary text, contents boundary, comment
        assert!(matches!(
            project_tree(
                &document,
                exact,
                &mut ObservationAllocationController::default()
            ),
            Ok(ObservationState::Captured(_))
        ));
        assert_eq!(
            project_tree(
                &document,
                exact - 1,
                &mut ObservationAllocationController::default()
            ),
            Ok(ObservationState::Incomplete {
                partial: ObservedTree::default(),
                reason: IncompleteObservationReason::StorageLimitExceeded {
                    retained: 0,
                    dropped: exact as u64,
                },
            })
        );
        assert!(matches!(
            project_tree(
                &document,
                0,
                &mut ObservationAllocationController::default()
            ),
            Ok(ObservationState::Incomplete {
                partial: ObservedTree { roots },
                ..
            }) if roots.is_empty()
        ));
    }

    #[test]
    fn qualified_attributes_do_not_consume_tree_structural_capacity() {
        let mut names = NameInterner::new();
        let div = names.intern_exact("div").unwrap();
        let lang = names.intern_exact("lang").unwrap();
        let href = names.intern_exact("href").unwrap();
        let element_name = ExpandedElementName::new(
            ElementNamespace::Html,
            names.resolve_local_name(div).unwrap(),
        );
        let attribute_sets = [
            Vec::new(),
            vec![ParserCreatedAttribute::new(
                QualifiedAttributeName::xml(names.resolve_local_name(lang).unwrap()),
                "en".to_owned(),
            )],
            vec![
                ParserCreatedAttribute::new(
                    QualifiedAttributeName::xml(names.resolve_local_name(lang).unwrap()),
                    "en".to_owned(),
                ),
                ParserCreatedAttribute::new(
                    QualifiedAttributeName::xlink(names.resolve_local_name(href).unwrap()),
                    "#target".to_owned(),
                ),
            ],
        ];
        for attributes in attribute_sets {
            let document = Node::Document {
                id: Id(1),
                doctype: None,
                children: vec![Node::from_element_parts(
                    Id(2),
                    element_name.clone(),
                    attributes,
                    Vec::new(),
                    None,
                    Vec::new(),
                )],
            };
            assert!(matches!(
                project_tree(
                    &document,
                    2,
                    &mut ObservationAllocationController::default()
                ),
                Ok(ObservationState::Captured(_))
            ));
            assert_eq!(
                project_tree(
                    &document,
                    1,
                    &mut ObservationAllocationController::default()
                ),
                Ok(ObservationState::Incomplete {
                    partial: ObservedTree::default(),
                    reason: IncompleteObservationReason::StorageLimitExceeded {
                        retained: 0,
                        dropped: 2,
                    },
                })
            );
        }
    }

    fn assert_tree_invariant_at_all_capacities(
        document: &Node,
        otherwise_required: usize,
        expected: ParserObservationInvariantError,
    ) {
        for capacity in [
            0,
            otherwise_required.saturating_sub(1),
            otherwise_required,
            usize::MAX,
        ] {
            assert_eq!(
                project_tree(
                    document,
                    capacity,
                    &mut ObservationAllocationController::default()
                ),
                Err(observation_invariant(expected)),
                "capacity {capacity} must not conceal malformed materialized state"
            );
        }
    }

    #[test]
    fn legacy_document_doctype_metadata_precedes_every_capacity_outcome() {
        let document = Node::Document {
            id: Id(1),
            doctype: Some("legacy".to_owned()),
            children: vec![Node::Text {
                id: Id(2),
                text: "x".to_owned(),
            }],
        };
        assert_tree_invariant_at_all_capacities(
            &document,
            2,
            ParserObservationInvariantError::UnexpectedLegacyDocumentDoctypeMetadata,
        );
    }

    #[test]
    fn canonical_tree_requires_exactly_one_document_root() {
        let root = Node::Text {
            id: Id(1),
            text: "not a document".to_owned(),
        };
        assert_tree_invariant_at_all_capacities(
            &root,
            1,
            ParserObservationInvariantError::CanonicalTreeRootNotDocument,
        );

        let nested_document = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Document {
                id: Id(2),
                doctype: None,
                children: Vec::new(),
            }],
        };
        assert_tree_invariant_at_all_capacities(
            &nested_document,
            2,
            ParserObservationInvariantError::CanonicalTreeRootNotDocument,
        );
    }

    #[test]
    fn missing_html_template_contents_precedes_every_capacity_outcome() {
        let mut names = NameInterner::new();
        let template = names.intern_exact("template").unwrap();
        let document = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::from_element_parts(
                Id(2),
                ExpandedElementName::new(
                    ElementNamespace::Html,
                    names.resolve_local_name(template).unwrap(),
                ),
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
            )],
        };
        assert_tree_invariant_at_all_capacities(
            &document,
            3,
            ParserObservationInvariantError::MissingHtmlTemplateContents,
        );
    }

    #[test]
    fn invalid_template_contents_kind_precedes_every_capacity_outcome() {
        let mut document = template_tree();
        let Node::Document { children, .. } = &mut document else {
            unreachable!();
        };
        let Node::Element { element } = &mut children[0] else {
            unreachable!();
        };
        element
            .template_contents_mut()
            .expect("template association")
            .force_unsupported_kind_for_conformance_test();
        assert_tree_invariant_at_all_capacities(
            &document,
            5,
            ParserObservationInvariantError::InvalidTemplateContentsKind,
        );
    }

    #[test]
    fn foreign_template_names_remain_ordinary_elements() {
        let mut names = NameInterner::new();
        let template = names.intern_exact("template").unwrap();
        for namespace in [ElementNamespace::Svg, ElementNamespace::MathMl] {
            let document = Node::Document {
                id: Id(1),
                doctype: None,
                children: vec![Node::from_element_parts(
                    Id(2),
                    ExpandedElementName::new(
                        namespace,
                        names.resolve_local_name(template).unwrap(),
                    ),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                )],
            };
            let state = project_tree(
                &document,
                2,
                &mut ObservationAllocationController::default(),
            )
            .unwrap();
            let ObservationState::Captured(tree) = state else {
                panic!("foreign template is an ordinary complete element");
            };
            let [ObservedTreeNode::Document { children }] = tree.roots.as_slice() else {
                panic!("document");
            };
            assert!(matches!(
                children.as_slice(),
                [ObservedTreeNode::Element {
                    namespace: actual,
                    local_name,
                    children,
                    ..
                }] if *actual == namespace && local_name == "template" && children.is_empty()
            ));
        }
    }

    #[test]
    fn synthetic_preflight_projection_mismatch_is_a_typed_invariant() {
        assert_eq!(
            validate_projected_unit_count(2, 1),
            Err(observation_invariant(
                ParserObservationInvariantError::CanonicalTreePreflightProjectionMismatch
            ))
        );
    }

    #[test]
    fn deep_projection_and_teardown_do_not_depend_on_native_recursion() {
        const DEPTH: usize = 12_000;
        let mut names = NameInterner::new();
        let div = names.intern_exact("div").unwrap();
        let expanded = ExpandedElementName::new(
            ElementNamespace::Html,
            names.resolve_local_name(div).unwrap(),
        );
        let mut child = Node::Text {
            id: Id((DEPTH + 2) as u32),
            text: "leaf".to_owned(),
        };
        for index in (0..DEPTH).rev() {
            child = Node::from_element_parts(
                Id((index + 2) as u32),
                expanded.clone(),
                Vec::new(),
                Vec::new(),
                None,
                vec![child],
            );
        }
        let mut document = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![child],
        };
        let state = project_tree(
            &document,
            DEPTH + 2,
            &mut ObservationAllocationController::default(),
        )
        .unwrap();
        let ObservationState::Captured(tree) = state else {
            panic!("captured");
        };
        assert_pure_deep_tree_shape(&tree, DEPTH);
        drop_observed_tree_iteratively(tree);
        drop_node_iteratively(&mut document);
    }

    fn assert_pure_deep_tree_shape(tree: &ObservedTree, expected_elements: usize) {
        let [ObservedTreeNode::Document { children }] = tree.roots.as_slice() else {
            panic!("exactly one document root");
        };
        let mut current = children.as_slice();
        let mut element_count = 0usize;
        let mut maximum_depth = 0usize;
        let mut structural_units = 1usize;
        loop {
            match current {
                [
                    ObservedTreeNode::Element {
                        namespace: ElementNamespace::Html,
                        local_name,
                        children,
                        ..
                    },
                ] if local_name == "div" => {
                    element_count += 1;
                    maximum_depth = element_count;
                    structural_units += 1;
                    current = children;
                }
                [ObservedTreeNode::Text { data }] if data == "leaf" => {
                    maximum_depth += 1;
                    structural_units += 1;
                    break;
                }
                _ => panic!("deep chain must preserve one source-ordered child at every depth"),
            }
        }
        assert_eq!(element_count, expected_elements);
        assert_eq!(maximum_depth, expected_elements + 1);
        assert_eq!(structural_units, expected_elements + 2);
    }

    fn drop_observed_tree_iteratively(tree: ObservedTree) {
        let mut stack = tree.roots;
        while let Some(mut node) = stack.pop() {
            match &mut node {
                ObservedTreeNode::Document { children }
                | ObservedTreeNode::Element { children, .. } => {
                    stack.extend(std::mem::take(children));
                }
                ObservedTreeNode::HtmlTemplateElement {
                    ordinary_children,
                    contents,
                    ..
                } => {
                    stack.extend(std::mem::take(ordinary_children));
                    stack.extend(std::mem::take(&mut contents.children));
                }
                ObservedTreeNode::DocumentType { .. }
                | ObservedTreeNode::Comment { .. }
                | ObservedTreeNode::Text { .. }
                | ObservedTreeNode::ProcessingInstruction { .. } => {}
            }
        }
    }

    fn drop_node_iteratively(document: &mut Node) {
        let (ordinary, template) = document.take_child_groups_for_iterative_drop();
        let mut stack = ordinary;
        if let Some(template) = template {
            stack.extend(template);
        }
        while let Some(mut node) = stack.pop() {
            let (ordinary, template) = node.take_child_groups_for_iterative_drop();
            stack.extend(ordinary);
            if let Some(template) = template {
                stack.extend(template);
            }
        }
    }
}
