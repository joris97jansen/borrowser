use crate::dom_patch::PatchKey;
use crate::types::{DocumentFragmentNode, Node, ParserCreatedFragmentKind};

use super::model::{PatchKind, PatchValidationArena};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FinalAuditSemanticTraversalAllocation;

#[derive(Clone, Copy)]
enum MaterializedRef<'a> {
    Node(&'a Node),
    Fragment(&'a DocumentFragmentNode),
}

#[derive(Clone, Copy)]
enum TraversalPhase {
    ComparePayload,
    OrdinaryChildren,
    TemplateContents,
    Complete,
}

#[derive(Clone, Copy)]
struct SemanticCompareFrame<'a> {
    arena_node: PatchKey,
    materialized_node: MaterializedRef<'a>,
    next_child_index: usize,
    phase: TraversalPhase,
}

impl PatchValidationArena {
    /// Compare the complete semantic patch-arena model with materialized DOM
    /// without snapshots, parser IDs, or recursive traversal. The stack keeps
    /// one frame per active ancestor and never retains all sibling pairs.
    pub(crate) fn semantic_equals_materialized_dom_for_final_audit(
        &self,
        document: &Node,
        reserve: &mut impl FnMut(crate::conformance::ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<bool, FinalAuditSemanticTraversalAllocation> {
        self.semantic_compare_for_final_audit(document, None, reserve)
    }

    #[cfg(test)]
    pub(crate) fn semantic_compare_depth_for_test(
        &self,
        document: &Node,
    ) -> Result<(bool, usize), FinalAuditSemanticTraversalAllocation> {
        let mut maximum_depth = 0usize;
        let mut reserve = |_| Ok(());
        let matches = self.semantic_compare_for_final_audit(
            document,
            Some(&mut maximum_depth),
            &mut reserve,
        )?;
        Ok((matches, maximum_depth))
    }

    fn semantic_compare_for_final_audit(
        &self,
        document: &Node,
        mut maximum_depth: Option<&mut usize>,
        reserve: &mut impl FnMut(crate::conformance::ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<bool, FinalAuditSemanticTraversalAllocation> {
        let Some(root) = self.root else {
            return Ok(false);
        };
        let mut stack = Vec::new();
        reserve(crate::conformance::ObservationReservationSite::FinalAuditSemanticTraversal)
            .map_err(|_| FinalAuditSemanticTraversalAllocation)?;
        stack
            .try_reserve(1)
            .map_err(|_| FinalAuditSemanticTraversalAllocation)?;
        stack.push(SemanticCompareFrame {
            arena_node: root,
            materialized_node: MaterializedRef::Node(document),
            next_child_index: 0,
            phase: TraversalPhase::ComparePayload,
        });
        record_depth(&mut maximum_depth, stack.len());

        while let Some(frame) = stack.last_mut() {
            let Some(arena) = self.nodes.get(&frame.arena_node) else {
                return Ok(false);
            };
            match frame.phase {
                TraversalPhase::ComparePayload => {
                    if !payload_matches(&arena.kind, frame.materialized_node) {
                        return Ok(false);
                    }
                    frame.phase = TraversalPhase::OrdinaryChildren;
                }
                TraversalPhase::OrdinaryChildren => {
                    let materialized_children = children(frame.materialized_node);
                    if arena.children.len() != materialized_children.len() {
                        return Ok(false);
                    }
                    if frame.next_child_index < arena.children.len() {
                        let index = frame.next_child_index;
                        frame.next_child_index = index
                            .checked_add(1)
                            .ok_or(FinalAuditSemanticTraversalAllocation)?;
                        let Some(arena_child) = arena.children.get(index).copied() else {
                            return Ok(false);
                        };
                        let Some(materialized_child) = materialized_children.get(index) else {
                            return Ok(false);
                        };
                        reserve(crate::conformance::ObservationReservationSite::FinalAuditSemanticTraversal)
                            .map_err(|_| FinalAuditSemanticTraversalAllocation)?;
                        stack
                            .try_reserve(1)
                            .map_err(|_| FinalAuditSemanticTraversalAllocation)?;
                        stack.push(SemanticCompareFrame {
                            arena_node: arena_child,
                            materialized_node: MaterializedRef::Node(materialized_child),
                            next_child_index: 0,
                            phase: TraversalPhase::ComparePayload,
                        });
                        record_depth(&mut maximum_depth, stack.len());
                    } else {
                        frame.phase = TraversalPhase::TemplateContents;
                    }
                }
                TraversalPhase::TemplateContents => {
                    let arena_contents = arena.template_contents();
                    let materialized_contents = match frame.materialized_node {
                        MaterializedRef::Node(Node::Element { element }) => {
                            element.template_contents()
                        }
                        MaterializedRef::Node(_) | MaterializedRef::Fragment(_) => None,
                    };
                    match (arena_contents, materialized_contents) {
                        (None, None) => frame.phase = TraversalPhase::Complete,
                        (Some(key), Some(contents)) => {
                            frame.phase = TraversalPhase::Complete;
                            reserve(crate::conformance::ObservationReservationSite::FinalAuditSemanticTraversal)
                                .map_err(|_| FinalAuditSemanticTraversalAllocation)?;
                            stack
                                .try_reserve(1)
                                .map_err(|_| FinalAuditSemanticTraversalAllocation)?;
                            stack.push(SemanticCompareFrame {
                                arena_node: key,
                                materialized_node: MaterializedRef::Fragment(contents),
                                next_child_index: 0,
                                phase: TraversalPhase::ComparePayload,
                            });
                            record_depth(&mut maximum_depth, stack.len());
                        }
                        (None, Some(_)) | (Some(_), None) => return Ok(false),
                    }
                }
                TraversalPhase::Complete => {
                    let _ = stack.pop();
                }
            }
        }
        Ok(true)
    }
}

fn record_depth(maximum: &mut Option<&mut usize>, current: usize) {
    if let Some(maximum) = maximum.as_deref_mut()
        && current > *maximum
    {
        *maximum = current;
    }
}

fn children(node: MaterializedRef<'_>) -> &[Node] {
    match node {
        MaterializedRef::Node(node) => match node.children() {
            Some(children) => children,
            None => &[],
        },
        MaterializedRef::Fragment(fragment) => fragment.children(),
    }
}

fn payload_matches(kind: &PatchKind, materialized: MaterializedRef<'_>) -> bool {
    match (kind, materialized) {
        (
            PatchKind::Document { doctype: expected },
            MaterializedRef::Node(Node::Document { doctype, .. }),
        ) => expected == doctype,
        (
            PatchKind::DocumentType {
                name: expected_name,
                public_id: expected_public,
                system_id: expected_system,
            },
            MaterializedRef::Node(Node::DocumentType {
                name,
                public_id,
                system_id,
                ..
            }),
        ) => expected_name == name && expected_public == public_id && expected_system == system_id,
        (
            PatchKind::Element {
                name,
                attributes,
                template_contents,
            },
            MaterializedRef::Node(Node::Element { element }),
        ) => {
            name == element.expanded_name()
                && attributes.as_slice() == element.attributes()
                && template_contents.is_some() == element.template_contents().is_some()
        }
        (PatchKind::DocumentFragment { kind, .. }, MaterializedRef::Fragment(fragment)) => {
            *kind == ParserCreatedFragmentKind::TemplateContents
                && fragment.kind() == ParserCreatedFragmentKind::TemplateContents
        }
        (PatchKind::Text { text: expected }, MaterializedRef::Node(Node::Text { text, .. })) => {
            expected == text
        }
        (
            PatchKind::Comment { text: expected },
            MaterializedRef::Node(Node::Comment { text, .. }),
        ) => expected == text,
        (
            PatchKind::ProcessingInstruction {
                target: expected_target,
                data: expected_data,
            },
            MaterializedRef::Node(Node::ProcessingInstruction {
                processing_instruction,
            }),
        ) => {
            expected_target == processing_instruction.target()
                && expected_data == processing_instruction.data()
        }
        _ => false,
    }
}
