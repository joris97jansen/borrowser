use crate::types::DocumentFragmentNode;
use crate::types::{Node, debug_assert_lowercase_atom};

/// One identity-bearing entry in the parser-created full model. Fragment roots
/// remain typed and are never exposed as ordinary `Node` children.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FullModelNodeRef<'a> {
    Node(&'a Node),
    DocumentFragment(&'a DocumentFragmentNode),
}

impl FullModelNodeRef<'_> {
    pub(crate) fn id(self) -> crate::types::Id {
        match self {
            Self::Node(node) => node.id(),
            Self::DocumentFragment(fragment) => fragment.id(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FullModelVisit<'a> {
    pub(crate) entry: FullModelNodeRef<'a>,
    /// Containing full-model entry. For a fragment root this is the host
    /// association, not an ordinary DOM parent edge.
    pub(crate) container: Option<crate::types::Id>,
    pub(crate) depth: usize,
}

pub(crate) struct FullModelPreorder<'a> {
    stack: Vec<FullModelVisit<'a>>,
}

impl<'a> FullModelPreorder<'a> {
    pub(crate) fn new(root: &'a Node) -> Self {
        Self {
            stack: vec![FullModelVisit {
                entry: FullModelNodeRef::Node(root),
                container: None,
                depth: 0,
            }],
        }
    }
}

impl<'a> Iterator for FullModelPreorder<'a> {
    type Item = FullModelVisit<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let visit = self.stack.pop()?;
        let child_depth = visit.depth.saturating_add(1);
        let container = Some(visit.entry.id());
        match visit.entry {
            FullModelNodeRef::Node(Node::Document { children, .. }) => {
                self.stack
                    .extend(children.iter().rev().map(|child| FullModelVisit {
                        entry: FullModelNodeRef::Node(child),
                        container,
                        depth: child_depth,
                    }));
            }
            FullModelNodeRef::Node(Node::Element { element }) => {
                // Ordinary children are pushed first so the association is
                // popped and visited before them.
                self.stack
                    .extend(element.children().iter().rev().map(|child| FullModelVisit {
                        entry: FullModelNodeRef::Node(child),
                        container,
                        depth: child_depth,
                    }));
                if let Some(contents) = element.template_contents() {
                    self.stack.push(FullModelVisit {
                        entry: FullModelNodeRef::DocumentFragment(contents),
                        container,
                        depth: child_depth,
                    });
                }
            }
            FullModelNodeRef::DocumentFragment(fragment) => {
                self.stack.extend(
                    fragment
                        .children()
                        .iter()
                        .rev()
                        .map(|child| FullModelVisit {
                            entry: FullModelNodeRef::Node(child),
                            container,
                            depth: child_depth,
                        }),
                );
            }
            FullModelNodeRef::Node(
                Node::DocumentType { .. } | Node::Text { .. } | Node::Comment { .. },
            ) => {}
        }
        Some(visit)
    }
}

pub(crate) fn full_model_preorder(root: &Node) -> FullModelPreorder<'_> {
    FullModelPreorder::new(root)
}

#[cfg(test)]
pub(crate) fn full_model_node_count(root: &Node) -> usize {
    full_model_preorder(root).count()
}

// Centralizes raw-pointer traversal handling for `assign_missing_ids_allow_collisions`.
#[cfg(any(test, all(feature = "test-harness", feature = "internal-api")))]
enum FullModelMutPtr {
    Node(*mut Node),
    Fragment(*mut DocumentFragmentNode),
}

#[cfg(any(test, all(feature = "test-harness", feature = "internal-api")))]
struct NodeStack {
    stack: Vec<FullModelMutPtr>,
}

#[cfg(any(test, all(feature = "test-harness", feature = "internal-api")))]
impl NodeStack {
    fn new(root: &mut Node) -> Self {
        Self {
            stack: {
                let mut stack = Vec::with_capacity(128);
                stack.push(FullModelMutPtr::Node(root as *mut Node));
                stack
            },
        }
    }

    fn pop_ptr(&mut self) -> Option<FullModelMutPtr> {
        self.stack.pop()
    }

    fn push_ptr(&mut self, ptr: FullModelMutPtr) {
        self.stack.push(ptr);
    }
}

/// Assigns missing IDs in depth-first, pre-order traversal (document/element before children).
///
/// ⚠️ Not patch-identity safe:
/// - Existing non-zero IDs are preserved.
/// - Collisions are not detected or resolved.
///
/// This is intended for tests or legacy callers that need to fill in missing IDs.
#[cfg(any(test, all(feature = "test-harness", feature = "internal-api")))]
pub(crate) fn assign_missing_ids_allow_collisions(root: &mut Node) {
    use crate::types::Id;
    let mut next = 1;
    let mut stack = NodeStack::new(root);

    while let Some(item) = stack.pop_ptr() {
        // SAFETY: pointers originate from ordinary node vectors or typed
        // `DocumentFragmentNode::children` vectors physically owned by `root`.
        // We never mutate either class of child vector during traversal, so no
        // backing storage reallocates. Only identity fields are changed, which
        // does not move nodes or fragment roots.
        match item {
            FullModelMutPtr::Node(node_ptr) => {
                let node = unsafe { &mut *node_ptr };
                if node.id() == Id::INVALID {
                    let id = Id(next);
                    next = next.checked_add(1).expect("node id overflow");
                    node.set_id(id);
                }

                match node {
                    Node::Document { children, .. } => {
                        for child in children.iter_mut().rev() {
                            stack.push_ptr(FullModelMutPtr::Node(child as *mut Node));
                        }
                    }
                    Node::Element { element } => {
                        for child in element.children_mut().iter_mut().rev() {
                            stack.push_ptr(FullModelMutPtr::Node(child as *mut Node));
                        }
                        if let Some(contents) = element.template_contents_mut() {
                            stack.push_ptr(FullModelMutPtr::Fragment(
                                contents as *mut DocumentFragmentNode,
                            ));
                        }
                    }
                    Node::DocumentType { .. } | Node::Text { .. } | Node::Comment { .. } => {}
                }
            }
            FullModelMutPtr::Fragment(fragment_ptr) => {
                let fragment = unsafe { &mut *fragment_ptr };
                if fragment.id() == Id::INVALID {
                    let id = Id(next);
                    next = next.checked_add(1).expect("node id overflow");
                    fragment.set_id(id);
                }
                for child in fragment.children_mut().iter_mut().rev() {
                    stack.push_ptr(FullModelMutPtr::Node(child as *mut Node));
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn find_node_by_id(node: &Node, id: crate::types::Id) -> Option<&Node> {
    let mut stack: Vec<&Node> = Vec::with_capacity(128);
    stack.push(node);

    while let Some(current) = stack.pop() {
        if current.id() == id {
            return Some(current);
        }

        if let Some(children) = current.children() {
            for child in children.iter().rev() {
                stack.push(child);
            }
        }
    }

    None
}

#[cfg(test)]
pub(crate) fn find_full_model_node_by_id(
    root: &Node,
    id: crate::types::Id,
) -> Option<FullModelNodeRef<'_>> {
    full_model_preorder(root)
        .map(|visit| visit.entry)
        .find(|entry| entry.id() == id)
}

pub fn is_non_rendering_element(node: &Node) -> bool {
    match node {
        Node::Element { element } => {
            let name = element.name();
            debug_assert_lowercase_atom(name, "non-rendering tag");
            name.as_ref() == "head"
                || name.as_ref() == "style"
                || name.as_ref() == "script"
                || name.as_ref() == "title"
                || name.as_ref() == "meta"
                || name.as_ref() == "link"
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FullModelNodeRef, assign_missing_ids_allow_collisions, find_full_model_node_by_id,
        find_node_by_id, full_model_preorder,
    };
    use crate::Node;
    use crate::types::{DocumentFragmentNode, Id};
    use std::sync::Arc;

    fn deep_document(depth: usize) -> Node {
        let mut current = crate::Node::from_element_parts(
            Id::INVALID,
            Arc::<str>::from("div"),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        );

        for _ in 1..depth {
            current = crate::Node::from_element_parts(
                Id::INVALID,
                Arc::<str>::from("div"),
                Vec::new(),
                Vec::new(),
                None,
                vec![current],
            );
        }

        Node::Document {
            id: Id::INVALID,
            doctype: None,
            children: vec![current],
        }
    }

    fn template_element(id: Id, contents: DocumentFragmentNode, children: Vec<Node>) -> Node {
        crate::Node::from_element_parts(
            id,
            Arc::from("template"),
            Vec::new(),
            Vec::new(),
            Some(Box::new(contents)),
            children,
        )
    }

    #[test]
    fn assign_missing_ids_is_iterative_and_preorder() {
        let depth = 10_000;
        let mut root = deep_document(depth);
        assign_missing_ids_allow_collisions(&mut root);

        let mut expected = 1u32;
        let mut current = &root;
        loop {
            assert_eq!(current.id(), Id(expected));
            expected += 1;

            if let Some(children) = current.children()
                && let Some(child) = children.first()
            {
                current = child;
                continue;
            }
            break;
        }

        assert_eq!(expected, (depth as u32) + 2);
    }

    #[test]
    fn find_node_by_id_is_iterative() {
        let depth = 10_000;
        let mut root = deep_document(depth);
        assign_missing_ids_allow_collisions(&mut root);

        let target = Id((depth / 2) as u32 + 1);
        let found = find_node_by_id(&root, target);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), target);
    }

    #[test]
    fn find_node_by_id_returns_none_when_missing() {
        let depth = 10_000;
        let mut root = deep_document(depth);
        assign_missing_ids_allow_collisions(&mut root);

        assert!(find_node_by_id(&root, Id(999_999_999)).is_none());
    }

    #[test]
    fn assign_missing_ids_preserves_source_order_for_siblings() {
        let mut root = Node::Document {
            id: Id::INVALID,
            doctype: None,
            children: vec![
                crate::Node::from_element_parts(
                    Id::INVALID,
                    Arc::<str>::from("a"),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                ),
                crate::Node::from_element_parts(
                    Id::INVALID,
                    Arc::<str>::from("b"),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                ),
                crate::Node::from_element_parts(
                    Id::INVALID,
                    Arc::<str>::from("c"),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                ),
            ],
        };

        assign_missing_ids_allow_collisions(&mut root);

        let children = match &root {
            Node::Document { children, .. } => children,
            _ => panic!("expected document root"),
        };

        assert_eq!(children[0].id(), Id(2));
        assert_eq!(children[1].id(), Id(3));
        assert_eq!(children[2].id(), Id(4));
    }

    #[test]
    fn assign_missing_ids_preserves_existing_ids() {
        let mut root = Node::Document {
            id: Id::INVALID,
            doctype: None,
            children: vec![
                crate::Node::from_element_parts(
                    Id(42),
                    Arc::<str>::from("div"),
                    Vec::new(),
                    Vec::new(),
                    None,
                    vec![Node::Text {
                        id: Id::INVALID,
                        text: "child".to_string(),
                    }],
                ),
                crate::Node::from_element_parts(
                    Id::INVALID,
                    Arc::<str>::from("span"),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                ),
            ],
        };

        assign_missing_ids_allow_collisions(&mut root);

        let children = match &root {
            Node::Document { children, .. } => children,
            _ => panic!("expected document root"),
        };

        assert_eq!(children[0].id(), Id(42));
        if let Node::Element { element } = &children[0] {
            assert_eq!(element.children()[0].id(), Id(2));
        }
        assert_eq!(children[1].id(), Id(3));
    }

    #[test]
    fn assign_missing_ids_allows_conflicting_existing_ids() {
        let mut root = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![crate::Node::from_element_parts(
                Id::INVALID,
                Arc::<str>::from("div"),
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
            )],
        };

        assign_missing_ids_allow_collisions(&mut root);

        let children = match &root {
            Node::Document { children, .. } => children,
            _ => panic!("expected document root"),
        };

        assert_eq!(root.id(), Id(1));
        // Collisions are allowed by contract; callers must not assume uniqueness.
        assert_eq!(children[0].id(), Id(1));
    }

    #[test]
    fn full_model_identity_assignment_uses_host_fragment_descendants_then_ordinary_children() {
        let nested = template_element(
            Id::INVALID,
            DocumentFragmentNode::new_template_contents(
                Id::INVALID,
                vec![Node::Text {
                    id: Id::INVALID,
                    text: "nested".to_string(),
                }],
            ),
            Vec::new(),
        );
        let mut root = Node::Document {
            id: Id::INVALID,
            doctype: None,
            children: vec![template_element(
                Id::INVALID,
                DocumentFragmentNode::new_template_contents(
                    Id::INVALID,
                    vec![crate::Node::from_element_parts(
                        Id::INVALID,
                        Arc::from("div"),
                        Vec::new(),
                        Vec::new(),
                        None,
                        vec![nested],
                    )],
                ),
                vec![Node::Comment {
                    id: Id::INVALID,
                    text: "legacy ordinary child".to_string(),
                }],
            )],
        };

        assign_missing_ids_allow_collisions(&mut root);
        let Node::Document { children, .. } = &root else {
            unreachable!()
        };
        let Node::Element { element: template } = &children[0] else {
            unreachable!()
        };
        assert_eq!(template.id(), Id(2));
        let contents = template.template_contents().expect("template contents");
        let ordinary_children = template.children();
        assert_eq!(contents.id(), Id(3));
        assert_eq!(contents.children()[0].id(), Id(4));
        let Node::Element { element: wrapper } = &contents.children()[0] else {
            unreachable!()
        };
        let Node::Element { element: nested } = &wrapper.children()[0] else {
            unreachable!()
        };
        assert_eq!(nested.id(), Id(5));
        let nested_contents = nested.template_contents().expect("nested contents");
        assert_eq!(nested_contents.id(), Id(6));
        assert_eq!(nested_contents.children()[0].id(), Id(7));
        assert_eq!(ordinary_children[0].id(), Id(8));

        assert!(find_node_by_id(&root, Id(3)).is_none());
        assert!(find_node_by_id(&root, Id(7)).is_none());
        assert!(matches!(
            find_full_model_node_by_id(&root, Id(3)),
            Some(FullModelNodeRef::DocumentFragment(_))
        ));
        assert!(matches!(
            find_full_model_node_by_id(&root, Id(7)),
            Some(FullModelNodeRef::Node(Node::Text { .. }))
        ));
    }

    #[test]
    fn host_and_fragment_identity_mutation_cannot_create_stale_host_references() {
        let mut root = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![template_element(
                Id(2),
                DocumentFragmentNode::new_template_contents(Id(3), Vec::new()),
                Vec::new(),
            )],
        };
        let Node::Document { children, .. } = &mut root else {
            unreachable!()
        };
        children[0].set_id(Id(20));
        let Node::Element { element } = &mut children[0] else {
            unreachable!()
        };
        let contents = element.template_contents_mut().expect("template contents");
        contents.set_id(Id(30));

        assert!(find_full_model_node_by_id(&root, Id(2)).is_none());
        assert!(find_full_model_node_by_id(&root, Id(3)).is_none());
        assert!(matches!(
            find_full_model_node_by_id(&root, Id(20)),
            Some(FullModelNodeRef::Node(Node::Element { .. }))
        ));
        assert!(matches!(
            find_full_model_node_by_id(&root, Id(30)),
            Some(FullModelNodeRef::DocumentFragment(_))
        ));
    }

    #[test]
    fn centralized_full_model_preorder_distinguishes_association_containers() {
        let root = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![template_element(
                Id(2),
                DocumentFragmentNode::new_template_contents(
                    Id(3),
                    vec![template_element(
                        Id(4),
                        DocumentFragmentNode::new_template_contents(
                            Id(5),
                            vec![Node::Text {
                                id: Id(6),
                                text: "nested".to_string(),
                            }],
                        ),
                        Vec::new(),
                    )],
                ),
                vec![Node::Comment {
                    id: Id(7),
                    text: "ordinary".to_string(),
                }],
            )],
        };

        let visits = full_model_preorder(&root)
            .map(|visit| (visit.entry.id(), visit.container, visit.depth))
            .collect::<Vec<_>>();
        assert_eq!(
            visits,
            vec![
                (Id(1), None, 0),
                (Id(2), Some(Id(1)), 1),
                (Id(3), Some(Id(2)), 2),
                (Id(4), Some(Id(3)), 3),
                (Id(5), Some(Id(4)), 4),
                (Id(6), Some(Id(5)), 5),
                (Id(7), Some(Id(2)), 2),
            ]
        );
    }
}
