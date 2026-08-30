use super::context::{SelectorDomAttribute, SelectorMatchDom};
use html::{ElementNamespace, ElementNode, Node, ParserCreatedAttribute, internal::Id};
use std::convert::Infallible;
use std::fmt::{self, Write};
use std::num::NonZeroU32;
use std::ops::Range;

/// Element identifier in one successfully built selector DOM projection.
///
/// This is a CSS-owned identity domain. The numeric value is one-based and is
/// derived from the element's zero-based position in the projection; it is not
/// an HTML node, patch, retained-render, or runtime identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SelectorDomElementId(NonZeroU32);

impl SelectorDomElementId {
    pub fn get(self) -> u32 {
        self.0.get()
    }

    fn zero_based_index(self) -> usize {
        self.get() as usize - 1
    }

    fn try_from_zero_based_index(
        index: usize,
        maximum: u32,
    ) -> Result<Self, SelectorDomBuildError> {
        let raw = index
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .filter(|value| value.get() <= maximum)
            .ok_or(SelectorDomBuildError::ElementIdRepresentationExhausted { maximum })?;
        Ok(Self(raw))
    }
}

/// HTML node kind reported when a document projection receives the wrong root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorDomNodeKind {
    Document,
    DocumentType,
    Element,
    Text,
    Comment,
    ProcessingInstruction,
}

impl SelectorDomNodeKind {
    fn of(node: &Node) -> Self {
        match node {
            Node::Document { .. } => Self::Document,
            Node::DocumentType { .. } => Self::DocumentType,
            Node::Element { .. } => Self::Element,
            Node::Text { .. } => Self::Text,
            Node::Comment { .. } => Self::Comment,
            Node::ProcessingInstruction { .. } => Self::ProcessingInstruction,
        }
    }
}

impl fmt::Display for SelectorDomNodeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Document => "document",
            Self::DocumentType => "document-type",
            Self::Element => "element",
            Self::Text => "text",
            Self::Comment => "comment",
            Self::ProcessingInstruction => "processing-instruction",
        })
    }
}

/// Heap-backed selector projection storage whose checked growth can fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorDomBuildStorage {
    PreflightTraversalStack,
    MaterializationTraversalStack,
    ElementRecords,
    DirectTextChildren,
}

impl fmt::Display for SelectorDomBuildStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PreflightTraversalStack => "preflight traversal stack",
            Self::MaterializationTraversalStack => "materialization traversal stack",
            Self::ElementRecords => "element records",
            Self::DirectTextChildren => "direct text children",
        })
    }
}

/// Typed failure to construct a selector DOM projection.
///
/// Reservation failures cover failures returned by Rust's fallible vector
/// reservation APIs. They do not promise recovery from a process-level
/// allocator abort or general out-of-memory termination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorDomBuildError {
    InvalidDocumentRoot {
        actual: SelectorDomNodeKind,
    },
    NestedDocument {
        depth: usize,
    },
    MultipleDocumentElements {
        first_child_index: usize,
        second_child_index: usize,
    },
    NonCanonicalHtmlElementLocalName {
        element_index: usize,
    },
    ElementIdRepresentationExhausted {
        maximum: u32,
    },
    ProjectionCapacityExceeded {
        storage: SelectorDomBuildStorage,
    },
    StorageReservationFailed {
        storage: SelectorDomBuildStorage,
    },
}

impl fmt::Display for SelectorDomBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDocumentRoot { actual } => {
                write!(
                    formatter,
                    "selector document projection requires a document root, got {actual}"
                )
            }
            Self::NestedDocument { depth } => {
                write!(
                    formatter,
                    "selector DOM contains a nested document at depth {depth}"
                )
            }
            Self::MultipleDocumentElements {
                first_child_index,
                second_child_index,
            } => write!(
                formatter,
                "selector document projection has multiple direct document elements at child indexes {first_child_index} and {second_child_index}"
            ),
            Self::NonCanonicalHtmlElementLocalName { element_index } => write!(
                formatter,
                "selector DOM element {element_index} has a non-canonical HTML local name"
            ),
            Self::ElementIdRepresentationExhausted { maximum } => write!(
                formatter,
                "selector DOM element identity representation exhausted at maximum {maximum}"
            ),
            Self::ProjectionCapacityExceeded { storage } => {
                write!(formatter, "selector DOM {storage} capacity was exceeded")
            }
            Self::StorageReservationFailed { storage } => {
                write!(formatter, "selector DOM could not reserve {storage}")
            }
        }
    }
}

impl std::error::Error for SelectorDomBuildError {}

/// Internal boundary used by style resolution to apply its element budget
/// during the projection preflight without making that policy a projection
/// validity rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoundedSelectorDomConstructionError {
    Build(SelectorDomBuildError),
    ElementLimitExceeded { limit: usize, observed: usize },
}

impl From<SelectorDomBuildError> for BoundedSelectorDomConstructionError {
    fn from(error: SelectorDomBuildError) -> Self {
        Self::Build(error)
    }
}

/// Compact indexed relationship record. Canonical names, namespaces,
/// attributes, and source DOM identity remain borrowed from `source` rather
/// than being duplicated in this hot per-element allocation.
#[derive(Debug)]
struct IndexedElement<'dom> {
    source: &'dom ElementNode,
    direct_text_range: Range<usize>,
    parent: Option<SelectorDomElementId>,
    previous_sibling: Option<SelectorDomElementId>,
    next_sibling: Option<SelectorDomElementId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectorDomProjectionRoot {
    Document {
        document_element: Option<SelectorDomElementId>,
    },
    ElementSubtree {
        root_element: SelectorDomElementId,
    },
}

/// Deterministic, fallibly constructed selector projection over an immutable
/// parser-created DOM.
///
/// The projection assigns CSS-local element identities in preorder, indexes
/// element-only parent/sibling axes, and retains exact ordinary direct-text
/// facts. Associated template-content fragments are excluded because both
/// construction passes inspect only [`ElementNode::children`].
pub struct SelectorDomIndex<'dom> {
    root: SelectorDomProjectionRoot,
    elements: Vec<IndexedElement<'dom>>,
    direct_text_children: Vec<&'dom str>,
}

impl fmt::Debug for SelectorDomIndex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectorDomIndex")
            .field("root", &self.root)
            .field("element_count", &self.elements.len())
            .field("direct_text_child_count", &self.direct_text_children.len())
            .finish()
    }
}

impl<'dom> SelectorDomIndex<'dom> {
    /// Builds the production document-rooted selector projection.
    pub fn try_from_document(root: &'dom Node) -> Result<Self, SelectorDomBuildError> {
        let input = document_input(root)?;
        try_build_unbounded(input, u32::MAX)
    }

    /// Test seam for an explicitly element-rooted closed projection. The root
    /// has no in-projection parent or siblings and is never the document
    /// element. Production compatibility code uses the bounded subtree path.
    #[cfg(test)]
    pub(crate) fn try_from_element_subtree(
        root: &'dom ElementNode,
    ) -> Result<Self, SelectorDomBuildError> {
        try_build_unbounded(ProjectionInput::ElementSubtree(root), u32::MAX)
    }

    pub(crate) fn try_from_document_with_element_limit(
        root: &'dom Node,
        limit: usize,
    ) -> Result<Self, BoundedSelectorDomConstructionError> {
        let input = document_input(root).map_err(BoundedSelectorDomConstructionError::Build)?;
        try_build_bounded(input, u32::MAX, limit)
    }

    pub(crate) fn try_from_element_subtree_with_element_limit(
        root: &'dom ElementNode,
        limit: usize,
    ) -> Result<Self, BoundedSelectorDomConstructionError> {
        try_build_bounded(ProjectionInput::ElementSubtree(root), u32::MAX, limit)
    }

    #[cfg(test)]
    pub(crate) fn try_from_document_with_max_element_id_for_test(
        root: &'dom Node,
        maximum: u32,
    ) -> Result<Self, SelectorDomBuildError> {
        let input = document_input(root)?;
        try_build_unbounded(input, maximum)
    }

    #[cfg(test)]
    pub(crate) const fn indexed_element_size_for_test() -> usize {
        std::mem::size_of::<IndexedElement<'static>>()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn elements(&self) -> SelectorDomElementIter {
        SelectorDomElementIter {
            next_index: 0,
            end_index: self.elements.len(),
        }
    }

    /// Maps a source DOM identity into this projection's distinct CSS-local
    /// identity domain.
    pub fn element_for_node_id(&self, node_id: Id) -> Option<SelectorDomElementId> {
        self.elements
            .iter()
            .position(|element| element.source.id() == node_id)
            .and_then(SelectorDomElementId::from_validated_index)
    }

    pub(crate) fn element_for_source(&self, source: &ElementNode) -> Option<SelectorDomElementId> {
        self.elements
            .iter()
            .position(|element| std::ptr::eq(element.source, source))
            .and_then(SelectorDomElementId::from_validated_index)
    }

    pub(crate) fn source_element(&self, element: SelectorDomElementId) -> &'dom ElementNode {
        self.record(element).source
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 3").expect("writing to String cannot fail");
        writeln!(&mut out, "selector-dom").expect("writing to String cannot fail");
        write_selector_dom_snapshot_body(&mut out, self, 0);
        out
    }

    fn record(&self, element: SelectorDomElementId) -> &IndexedElement<'dom> {
        // `SelectorDomElementId` is privately constructed and every public ID
        // originates from this validated projection.
        self.elements
            .get(element.zero_based_index())
            .expect("builder-created selector element ID must address its projection")
    }

    fn first_element_child_id(
        &self,
        element: SelectorDomElementId,
    ) -> Option<SelectorDomElementId> {
        let child_index = element.get() as usize;
        let child = self.elements.get(child_index)?;
        (child.parent == Some(element))
            .then(|| SelectorDomElementId::from_validated_index(child_index))
            .flatten()
    }
}

impl SelectorDomElementId {
    fn from_validated_index(index: usize) -> Option<Self> {
        index
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .map(Self)
    }
}

fn selector_dom_attribute(attribute: &ParserCreatedAttribute) -> SelectorDomAttribute<'_> {
    SelectorDomAttribute::new(
        attribute.namespace(),
        attribute.local_name(),
        attribute.value(),
    )
}

impl<'dom> SelectorMatchDom for SelectorDomIndex<'dom> {
    type ElementId = SelectorDomElementId;

    type AttributeIter<'a>
        = std::iter::Map<
        std::slice::Iter<'a, ParserCreatedAttribute>,
        fn(&'a ParserCreatedAttribute) -> SelectorDomAttribute<'a>,
    >
    where
        Self: 'a;

    type DirectTextChildIter<'a>
        = std::iter::Copied<std::slice::Iter<'a, &'a str>>
    where
        Self: 'a;

    fn document_element(&self) -> Option<Self::ElementId> {
        match self.root {
            SelectorDomProjectionRoot::Document { document_element } => document_element,
            SelectorDomProjectionRoot::ElementSubtree { .. } => None,
        }
    }

    fn parent_element(&self, element: Self::ElementId) -> Option<Self::ElementId> {
        self.record(element).parent
    }

    fn previous_sibling_element(&self, element: Self::ElementId) -> Option<Self::ElementId> {
        self.record(element).previous_sibling
    }

    fn next_sibling_element(&self, element: Self::ElementId) -> Option<Self::ElementId> {
        self.record(element).next_sibling
    }

    fn first_element_child(&self, element: Self::ElementId) -> Option<Self::ElementId> {
        self.first_element_child_id(element)
    }

    fn element_local_name(&self, element: Self::ElementId) -> &str {
        self.record(element).source.name()
    }

    fn element_namespace(&self, element: Self::ElementId) -> ElementNamespace {
        self.record(element).source.namespace()
    }

    fn attributes(&self, element: Self::ElementId) -> Self::AttributeIter<'_> {
        self.record(element)
            .source
            .attributes()
            .iter()
            .map(selector_dom_attribute as fn(&ParserCreatedAttribute) -> SelectorDomAttribute<'_>)
    }

    fn direct_text_children(&self, element: Self::ElementId) -> Self::DirectTextChildIter<'_> {
        self.direct_text_children[self.record(element).direct_text_range.clone()]
            .iter()
            .copied()
    }
}

/// Document-order iterator over [`SelectorDomElementId`] values.
pub struct SelectorDomElementIter {
    next_index: usize,
    end_index: usize,
}

impl SelectorDomElementIter {
    #[cfg(test)]
    pub(crate) fn for_validated_bounds_for_test(
        next_index: usize,
        end_index: usize,
    ) -> Option<Self> {
        (next_index <= end_index && u32::try_from(end_index).is_ok()).then_some(Self {
            next_index,
            end_index,
        })
    }
}

impl Iterator for SelectorDomElementIter {
    type Item = SelectorDomElementId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.end_index {
            return None;
        }

        let index = self.next_index;
        self.next_index += 1;
        // The builder rejects any projection whose last index cannot be
        // represented, so this assertion cannot be influenced by accepted
        // production input.
        Some(
            SelectorDomElementId::from_validated_index(index)
                .expect("validated selector projection index must remain representable"),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SelectorDomElementIter {
    fn len(&self) -> usize {
        self.end_index - self.next_index
    }
}

pub(crate) fn write_selector_dom_snapshot_body(
    out: &mut String,
    index: &SelectorDomIndex<'_>,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    match index.root {
        SelectorDomProjectionRoot::Document { document_element } => {
            writeln!(out, "{indent_str}projection: document").expect("write snapshot");
            write!(out, "{indent_str}document-element: ").expect("write snapshot");
            write_optional_element_id(out, document_element);
            writeln!(out).expect("write snapshot");
        }
        SelectorDomProjectionRoot::ElementSubtree { root_element } => {
            writeln!(out, "{indent_str}projection: element-subtree").expect("write snapshot");
            writeln!(out, "{indent_str}document-element: none").expect("write snapshot");
            writeln!(out, "{indent_str}subtree-root: {}", root_element.get())
                .expect("write snapshot");
        }
    }
    writeln!(out, "{indent_str}elements: {}", index.len()).expect("write snapshot");

    for (element_index, element_id) in index.elements().enumerate() {
        let record = index.record(element_id);
        write!(
            out,
            "{indent_str}element[{element_index}]: id={} namespace={} local={:?} parent=",
            element_id.get(),
            record.source.namespace().snapshot_name(),
            record.source.name()
        )
        .expect("write snapshot");
        write_optional_element_id(out, record.parent);
        write!(out, " prev-sibling=").expect("write snapshot");
        write_optional_element_id(out, record.previous_sibling);
        write!(out, " next-sibling=").expect("write snapshot");
        write_optional_element_id(out, record.next_sibling);
        write!(out, " first-child=").expect("write snapshot");
        write_optional_element_id(out, index.first_element_child_id(element_id));
        writeln!(out).expect("write snapshot");

        for (attribute_index, attribute) in record.source.attributes().iter().enumerate() {
            writeln!(
                out,
                "{indent_str}  attribute[{attribute_index}]: namespace={} local={:?} value={:?}",
                attribute.namespace().snapshot_name(),
                attribute.local_name(),
                attribute.value()
            )
            .expect("write snapshot");
        }
        for (text_index, text) in index.direct_text_children(element_id).enumerate() {
            writeln!(out, "{indent_str}  direct-text[{text_index}]: {text:?}")
                .expect("write snapshot");
        }
    }
}

fn write_optional_element_id(out: &mut String, element: Option<SelectorDomElementId>) {
    match element {
        Some(element) => write!(out, "{}", element.get()).expect("write snapshot"),
        None => write!(out, "none").expect("write snapshot"),
    }
}

#[derive(Clone, Copy)]
enum ProjectionInput<'dom> {
    Document(&'dom [Node]),
    ElementSubtree(&'dom ElementNode),
}

fn document_input(root: &Node) -> Result<ProjectionInput<'_>, SelectorDomBuildError> {
    match root {
        Node::Document { children, .. } => Ok(ProjectionInput::Document(children)),
        _ => Err(SelectorDomBuildError::InvalidDocumentRoot {
            actual: SelectorDomNodeKind::of(root),
        }),
    }
}

#[derive(Clone, Copy)]
enum ScanParentKind {
    Document,
    Element,
}

struct PreflightFrame<'dom> {
    children: &'dom [Node],
    next_child_index: usize,
    child_depth: usize,
    parent_kind: ScanParentKind,
}

struct MaterializationFrame<'dom> {
    children: &'dom [Node],
    next_child_index: usize,
    child_depth: usize,
    parent: MaterializationParent,
    previous_element_child: Option<SelectorDomElementId>,
}

struct MaterializationChild<'dom> {
    node: &'dom Node,
    child_index: usize,
    depth: usize,
    parent: MaterializationParent,
    previous_sibling: Option<SelectorDomElementId>,
}

#[derive(Clone, Copy)]
enum MaterializationParent {
    Document,
    Element(SelectorDomElementId),
}

#[derive(Clone, Copy, Debug)]
struct PreflightCounts {
    elements: usize,
    direct_text_children: usize,
    maximum_stack_depth: usize,
}

enum ConstructionError<E> {
    Build(SelectorDomBuildError),
    ElementBudget(E),
}

impl<E> From<SelectorDomBuildError> for ConstructionError<E> {
    fn from(error: SelectorDomBuildError) -> Self {
        Self::Build(error)
    }
}

#[derive(Clone, Copy)]
struct ElementLimitExceeded {
    limit: usize,
    observed: usize,
}

fn try_build_unbounded<'dom>(
    input: ProjectionInput<'dom>,
    maximum_element_id: u32,
) -> Result<SelectorDomIndex<'dom>, SelectorDomBuildError> {
    match try_build_with_element_budget(input, maximum_element_id, |_| Ok::<(), Infallible>(())) {
        Ok(index) => Ok(index),
        Err(ConstructionError::Build(error)) => Err(error),
        Err(ConstructionError::ElementBudget(never)) => match never {},
    }
}

fn try_build_bounded<'dom>(
    input: ProjectionInput<'dom>,
    maximum_element_id: u32,
    limit: usize,
) -> Result<SelectorDomIndex<'dom>, BoundedSelectorDomConstructionError> {
    let counts = try_preflight_bounded(input, maximum_element_id, limit)?;
    materialize(input, maximum_element_id, counts)
        .map_err(BoundedSelectorDomConstructionError::Build)
}

fn try_preflight_bounded(
    input: ProjectionInput<'_>,
    maximum_element_id: u32,
    limit: usize,
) -> Result<PreflightCounts, BoundedSelectorDomConstructionError> {
    match preflight(input, maximum_element_id, &mut |observed| {
        if observed > limit {
            Err(ElementLimitExceeded { limit, observed })
        } else {
            Ok(())
        }
    }) {
        Ok(counts) => Ok(counts),
        Err(ConstructionError::Build(error)) => {
            Err(BoundedSelectorDomConstructionError::Build(error))
        }
        Err(ConstructionError::ElementBudget(error)) => {
            Err(BoundedSelectorDomConstructionError::ElementLimitExceeded {
                limit: error.limit,
                observed: error.observed,
            })
        }
    }
}

fn try_build_with_element_budget<'dom, E>(
    input: ProjectionInput<'dom>,
    maximum_element_id: u32,
    mut check_element_budget: impl FnMut(usize) -> Result<(), E>,
) -> Result<SelectorDomIndex<'dom>, ConstructionError<E>> {
    let counts = preflight(input, maximum_element_id, &mut check_element_budget)?;
    materialize(input, maximum_element_id, counts).map_err(ConstructionError::Build)
}

fn preflight<E>(
    input: ProjectionInput<'_>,
    maximum_element_id: u32,
    check_element_budget: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<PreflightCounts, ConstructionError<E>> {
    let mut element_count = 0usize;
    let mut direct_text_count = 0usize;
    let mut first_document_element_child_index = None;

    if let ProjectionInput::ElementSubtree(root) = input {
        validate_canonical_element(root, 0)?;
        element_count =
            checked_next_element_count(element_count, maximum_element_id, check_element_budget)?;
    }

    let (root_children, parent_kind) = match input {
        ProjectionInput::Document(children) => (children, ScanParentKind::Document),
        ProjectionInput::ElementSubtree(root) => (root.children(), ScanParentKind::Element),
    };

    let mut stack = Vec::new();
    try_push(
        &mut stack,
        PreflightFrame {
            children: root_children,
            next_child_index: 0,
            child_depth: 1,
            parent_kind,
        },
        SelectorDomBuildStorage::PreflightTraversalStack,
    )?;
    let mut maximum_stack_depth = stack.len();

    while !stack.is_empty() {
        let Some((child, child_index, child_depth, parent_kind)) =
            next_preflight_child(&mut stack)?
        else {
            continue;
        };

        match child {
            Node::Document { .. } => {
                return Err(SelectorDomBuildError::NestedDocument { depth: child_depth }.into());
            }
            Node::Element { element } => {
                if matches!(parent_kind, ScanParentKind::Document) {
                    if let Some(first_child_index) = first_document_element_child_index {
                        return Err(SelectorDomBuildError::MultipleDocumentElements {
                            first_child_index,
                            second_child_index: child_index,
                        }
                        .into());
                    }
                    first_document_element_child_index = Some(child_index);
                }

                validate_canonical_element(element, element_count)?;
                element_count = checked_next_element_count(
                    element_count,
                    maximum_element_id,
                    check_element_budget,
                )?;

                if !element.children().is_empty() {
                    let grandchild_depth = child_depth.checked_add(1).ok_or(
                        SelectorDomBuildError::ProjectionCapacityExceeded {
                            storage: SelectorDomBuildStorage::PreflightTraversalStack,
                        },
                    )?;
                    try_push(
                        &mut stack,
                        PreflightFrame {
                            children: element.children(),
                            next_child_index: 0,
                            child_depth: grandchild_depth,
                            parent_kind: ScanParentKind::Element,
                        },
                        SelectorDomBuildStorage::PreflightTraversalStack,
                    )?;
                    maximum_stack_depth = maximum_stack_depth.max(stack.len());
                }
            }
            Node::Text { .. } if matches!(parent_kind, ScanParentKind::Element) => {
                direct_text_count = direct_text_count.checked_add(1).ok_or(
                    SelectorDomBuildError::ProjectionCapacityExceeded {
                        storage: SelectorDomBuildStorage::DirectTextChildren,
                    },
                )?;
            }
            Node::Text { .. }
            | Node::Comment { .. }
            | Node::ProcessingInstruction { .. }
            | Node::DocumentType { .. } => {}
        }
    }

    Ok(PreflightCounts {
        elements: element_count,
        direct_text_children: direct_text_count,
        maximum_stack_depth,
    })
}

fn checked_next_element_count<E>(
    current: usize,
    maximum_element_id: u32,
    check_element_budget: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<usize, ConstructionError<E>> {
    let next =
        current
            .checked_add(1)
            .ok_or(SelectorDomBuildError::ElementIdRepresentationExhausted {
                maximum: maximum_element_id,
            })?;
    SelectorDomElementId::try_from_zero_based_index(current, maximum_element_id)?;
    check_element_budget(next).map_err(ConstructionError::ElementBudget)?;
    Ok(next)
}

fn next_preflight_child<'dom>(
    stack: &mut Vec<PreflightFrame<'dom>>,
) -> Result<Option<(&'dom Node, usize, usize, ScanParentKind)>, SelectorDomBuildError> {
    let Some(frame) = stack.last_mut() else {
        return Ok(None);
    };
    if frame.next_child_index >= frame.children.len() {
        stack.pop();
        return Ok(None);
    }

    let child_index = frame.next_child_index;
    frame.next_child_index = frame.next_child_index.checked_add(1).ok_or(
        SelectorDomBuildError::ProjectionCapacityExceeded {
            storage: SelectorDomBuildStorage::PreflightTraversalStack,
        },
    )?;
    Ok(Some((
        &frame.children[child_index],
        child_index,
        frame.child_depth,
        frame.parent_kind,
    )))
}

fn materialize<'dom>(
    input: ProjectionInput<'dom>,
    maximum_element_id: u32,
    counts: PreflightCounts,
) -> Result<SelectorDomIndex<'dom>, SelectorDomBuildError> {
    let mut elements = Vec::new();
    try_reserve_exact(
        &mut elements,
        counts.elements,
        SelectorDomBuildStorage::ElementRecords,
    )?;
    let mut direct_text_children = Vec::new();
    try_reserve_exact(
        &mut direct_text_children,
        counts.direct_text_children,
        SelectorDomBuildStorage::DirectTextChildren,
    )?;
    let mut stack = Vec::new();
    try_reserve_exact(
        &mut stack,
        counts.maximum_stack_depth,
        SelectorDomBuildStorage::MaterializationTraversalStack,
    )?;

    let (initial_frame, subtree_root) = match input {
        ProjectionInput::Document(children) => (
            MaterializationFrame {
                children,
                next_child_index: 0,
                child_depth: 1,
                parent: MaterializationParent::Document,
                previous_element_child: None,
            },
            None,
        ),
        ProjectionInput::ElementSubtree(root) => {
            let root_id = materialize_element(
                root,
                None,
                None,
                maximum_element_id,
                &mut elements,
                &mut direct_text_children,
            )?;
            (
                MaterializationFrame {
                    children: root.children(),
                    next_child_index: 0,
                    child_depth: 1,
                    parent: MaterializationParent::Element(root_id),
                    previous_element_child: None,
                },
                Some(root_id),
            )
        }
    };
    try_push(
        &mut stack,
        initial_frame,
        SelectorDomBuildStorage::MaterializationTraversalStack,
    )?;

    let mut document_element = None;
    let mut first_document_element_child_index = None;

    while !stack.is_empty() {
        let Some(child) = next_materialization_child(&mut stack)? else {
            continue;
        };

        match child.node {
            Node::Document { .. } => {
                return Err(SelectorDomBuildError::NestedDocument { depth: child.depth });
            }
            Node::Element { element } => {
                if matches!(child.parent, MaterializationParent::Document) {
                    if let Some(first_child_index) = first_document_element_child_index {
                        return Err(SelectorDomBuildError::MultipleDocumentElements {
                            first_child_index,
                            second_child_index: child.child_index,
                        });
                    }
                    first_document_element_child_index = Some(child.child_index);
                }

                let parent_element = match child.parent {
                    MaterializationParent::Document => None,
                    MaterializationParent::Element(parent) => Some(parent),
                };
                let element_id = materialize_element(
                    element,
                    parent_element,
                    child.previous_sibling,
                    maximum_element_id,
                    &mut elements,
                    &mut direct_text_children,
                )?;
                if matches!(child.parent, MaterializationParent::Document) {
                    document_element = Some(element_id);
                }
                stack
                    .last_mut()
                    .expect("the current materialization frame remains present")
                    .previous_element_child = Some(element_id);

                if !element.children().is_empty() {
                    let grandchild_depth = child.depth.checked_add(1).ok_or(
                        SelectorDomBuildError::ProjectionCapacityExceeded {
                            storage: SelectorDomBuildStorage::MaterializationTraversalStack,
                        },
                    )?;
                    try_push(
                        &mut stack,
                        MaterializationFrame {
                            children: element.children(),
                            next_child_index: 0,
                            child_depth: grandchild_depth,
                            parent: MaterializationParent::Element(element_id),
                            previous_element_child: None,
                        },
                        SelectorDomBuildStorage::MaterializationTraversalStack,
                    )?;
                }
            }
            Node::Text { .. }
            | Node::Comment { .. }
            | Node::ProcessingInstruction { .. }
            | Node::DocumentType { .. } => {}
        }
    }

    debug_assert_eq!(elements.len(), counts.elements);
    debug_assert_eq!(direct_text_children.len(), counts.direct_text_children);

    let root = match subtree_root {
        Some(root_element) => SelectorDomProjectionRoot::ElementSubtree { root_element },
        None => SelectorDomProjectionRoot::Document { document_element },
    };
    Ok(SelectorDomIndex {
        root,
        elements,
        direct_text_children,
    })
}

fn next_materialization_child<'dom>(
    stack: &mut Vec<MaterializationFrame<'dom>>,
) -> Result<Option<MaterializationChild<'dom>>, SelectorDomBuildError> {
    let Some(frame) = stack.last_mut() else {
        return Ok(None);
    };
    if frame.next_child_index >= frame.children.len() {
        stack.pop();
        return Ok(None);
    }

    let child_index = frame.next_child_index;
    frame.next_child_index = frame.next_child_index.checked_add(1).ok_or(
        SelectorDomBuildError::ProjectionCapacityExceeded {
            storage: SelectorDomBuildStorage::MaterializationTraversalStack,
        },
    )?;
    Ok(Some(MaterializationChild {
        node: &frame.children[child_index],
        child_index,
        depth: frame.child_depth,
        parent: frame.parent,
        previous_sibling: frame.previous_element_child,
    }))
}

fn materialize_element<'dom>(
    element: &'dom ElementNode,
    parent: Option<SelectorDomElementId>,
    previous_sibling: Option<SelectorDomElementId>,
    maximum_element_id: u32,
    elements: &mut Vec<IndexedElement<'dom>>,
    direct_text_children: &mut Vec<&'dom str>,
) -> Result<SelectorDomElementId, SelectorDomBuildError> {
    validate_canonical_element(element, elements.len())?;
    let element_id =
        SelectorDomElementId::try_from_zero_based_index(elements.len(), maximum_element_id)?;

    // Owner-contiguous text invariant: all ordinary direct text children are
    // appended, in direct-child order, before any descendant frame is pushed.
    // Descendant text therefore cannot interleave with this half-open range.
    let direct_text_start = direct_text_children.len();
    for child in element.children() {
        if let Node::Text { text, .. } = child {
            try_push(
                direct_text_children,
                text.as_str(),
                SelectorDomBuildStorage::DirectTextChildren,
            )?;
        }
    }
    let direct_text_end = direct_text_children.len();

    try_reserve_one(elements, SelectorDomBuildStorage::ElementRecords)?;
    if let Some(previous_sibling) = previous_sibling {
        // A previous sibling ID is created by this same materialization pass
        // before the current record, so its addressability is an internal
        // builder invariant rather than an accepted-input assumption.
        elements[previous_sibling.zero_based_index()].next_sibling = Some(element_id);
    }
    elements.push(IndexedElement {
        source: element,
        direct_text_range: direct_text_start..direct_text_end,
        parent,
        previous_sibling,
        next_sibling: None,
    });
    Ok(element_id)
}

fn validate_canonical_element(
    element: &ElementNode,
    element_index: usize,
) -> Result<(), SelectorDomBuildError> {
    let local_name = element.name();
    if element.namespace() == ElementNamespace::Html
        && local_name.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(SelectorDomBuildError::NonCanonicalHtmlElementLocalName { element_index });
    }
    Ok(())
}

fn try_push<T>(
    storage: &mut Vec<T>,
    value: T,
    site: SelectorDomBuildStorage,
) -> Result<(), SelectorDomBuildError> {
    try_reserve_one(storage, site)?;
    storage.push(value);
    Ok(())
}

fn try_reserve_one<T>(
    storage: &mut Vec<T>,
    site: SelectorDomBuildStorage,
) -> Result<(), SelectorDomBuildError> {
    storage
        .len()
        .checked_add(1)
        .ok_or(SelectorDomBuildError::ProjectionCapacityExceeded { storage: site })?;
    storage
        .try_reserve(1)
        .map_err(|_| SelectorDomBuildError::StorageReservationFailed { storage: site })
}

fn try_reserve_exact<T>(
    storage: &mut Vec<T>,
    additional: usize,
    site: SelectorDomBuildStorage,
) -> Result<(), SelectorDomBuildError> {
    storage
        .len()
        .checked_add(additional)
        .ok_or(SelectorDomBuildError::ProjectionCapacityExceeded { storage: site })?;
    storage
        .try_reserve_exact(additional)
        .map_err(|_| SelectorDomBuildError::StorageReservationFailed { storage: site })
}
