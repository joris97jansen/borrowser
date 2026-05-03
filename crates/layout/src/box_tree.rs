use crate::replaced::intrinsic::IntrinsicSize;
use crate::{
    BoxKind, ListMarker, ReplacedElementInfoProvider, ReplacedKind, box_kind_debug_label,
    classify_replaced_kind, intrinsic_size_debug_label, list_marker_debug_label, node_debug_label,
    replaced_kind_debug_label,
};
use css::{ComputedStyle, Display, StyledNode};
use html::{Node, internal::Id};
use std::fmt::{self, Write};

/// Stable index for a generated layout box in a frame-local box tree.
///
/// Box IDs are deterministic for a fixed style tree and generation environment:
/// nodes are assigned in preorder as they are generated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoxId(usize);

impl BoxId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// High-level reason a box exists in the generated box tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxGenerationRole {
    DocumentRoot,
    DocumentElement,
    OrdinaryElement,
    TextRun,
    Anonymous(AnonymousBoxKind),
    Marker,
}

/// Reserved anonymous-box categories for later Milestone W generation rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnonymousBoxKind {
    Block,
    Inline,
}

/// Source relationship for a generated layout box.
///
/// Current W2 generation only creates DOM-backed boxes. Anonymous and marker
/// variants are part of the representation now so future box-generation work
/// does not need to make `LayoutBox` pretend every box is a DOM node.
#[derive(Clone, Copy)]
pub enum BoxSource<'style_tree, 'dom> {
    DomNode(&'style_tree StyledNode<'dom>),
    Anonymous {
        parent: &'style_tree StyledNode<'dom>,
        kind: AnonymousBoxKind,
    },
    Marker {
        list_item: &'style_tree StyledNode<'dom>,
    },
}

impl fmt::Debug for BoxSource<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            BoxSource::DomNode(node) => f
                .debug_struct("DomNode")
                .field("node_id", &node.node_id)
                .finish(),
            BoxSource::Anonymous { parent, kind } => f
                .debug_struct("Anonymous")
                .field("parent_node_id", &parent.node_id)
                .field("kind", &kind)
                .finish(),
            BoxSource::Marker { list_item } => f
                .debug_struct("Marker")
                .field("list_item_node_id", &list_item.node_id)
                .finish(),
        }
    }
}

impl<'style_tree, 'dom> BoxSource<'style_tree, 'dom> {
    pub fn direct_styled_node(self) -> Option<&'style_tree StyledNode<'dom>> {
        match self {
            BoxSource::DomNode(node) => Some(node),
            BoxSource::Anonymous { .. } | BoxSource::Marker { .. } => None,
        }
    }

    pub fn anchor_styled_node(self) -> &'style_tree StyledNode<'dom> {
        match self {
            BoxSource::DomNode(node) => node,
            BoxSource::Anonymous { parent, .. } => parent,
            BoxSource::Marker { list_item } => list_item,
        }
    }

    pub fn direct_node_id(self) -> Option<Id> {
        self.direct_styled_node().map(|node| node.node_id)
    }

    pub fn anchor_node_id(self) -> Id {
        self.anchor_styled_node().node_id
    }
}

/// A generated layout box node before final geometry is computed.
pub struct BoxNode<'style_tree, 'dom> {
    id: BoxId,
    parent: Option<BoxId>,
    children: Vec<BoxId>,
    role: BoxGenerationRole,
    kind: BoxKind,
    source: BoxSource<'style_tree, 'dom>,
    style: &'style_tree ComputedStyle,
    display: Display,
    list_marker: Option<ListMarker>,
    replaced: Option<ReplacedKind>,
    replaced_intrinsic: Option<IntrinsicSize>,
}

impl fmt::Debug for BoxNode<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxNode")
            .field("id", &self.id)
            .field("parent", &self.parent)
            .field("children", &self.children)
            .field("role", &self.role)
            .field("kind", &self.kind)
            .field("source", &self.source)
            .field("display", &self.display)
            .field("list_marker", &self.list_marker)
            .field("replaced", &self.replaced)
            .field("replaced_intrinsic", &self.replaced_intrinsic)
            .finish()
    }
}

impl<'style_tree, 'dom> BoxNode<'style_tree, 'dom> {
    pub fn id(&self) -> BoxId {
        self.id
    }

    pub fn parent(&self) -> Option<BoxId> {
        self.parent
    }

    pub fn children(&self) -> &[BoxId] {
        &self.children
    }

    pub fn role(&self) -> BoxGenerationRole {
        self.role
    }

    pub fn kind(&self) -> BoxKind {
        self.kind
    }

    pub fn source(&self) -> BoxSource<'style_tree, 'dom> {
        self.source
    }

    pub fn style(&self) -> &'style_tree ComputedStyle {
        self.style
    }

    pub fn display(&self) -> Display {
        self.display
    }

    pub fn list_marker(&self) -> Option<ListMarker> {
        self.list_marker
    }

    pub fn replaced(&self) -> Option<ReplacedKind> {
        self.replaced
    }

    pub fn replaced_intrinsic(&self) -> Option<IntrinsicSize> {
        self.replaced_intrinsic
    }

    pub fn direct_node_id(&self) -> Option<Id> {
        self.source.direct_node_id()
    }

    pub fn anchor_node_id(&self) -> Id {
        self.source.anchor_node_id()
    }
}

/// Explicit generated box tree for one layout pass.
pub struct BoxTree<'style_tree, 'dom> {
    root: BoxId,
    nodes: Vec<BoxNode<'style_tree, 'dom>>,
}

impl fmt::Debug for BoxTree<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxTree")
            .field("root", &self.root)
            .field("nodes", &self.nodes)
            .finish()
    }
}

impl<'style_tree, 'dom> BoxTree<'style_tree, 'dom> {
    pub fn generate(
        root: &'style_tree StyledNode<'dom>,
        replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    ) -> Self {
        let mut builder = BoxTreeBuilder { nodes: Vec::new() };
        let root = builder
            .build_styled_subtree(root, None, replaced_info)
            .unwrap_or_else(|| builder.push_fallback_root(root));

        Self {
            root,
            nodes: builder.nodes,
        }
    }

    pub fn root_id(&self) -> BoxId {
        self.root
    }

    pub fn root(&self) -> &BoxNode<'style_tree, 'dom> {
        self.node(self.root)
    }

    pub fn node(&self, id: BoxId) -> &BoxNode<'style_tree, 'dom> {
        &self.nodes[id.index()]
    }

    pub fn nodes(&self) -> &[BoxNode<'style_tree, 'dom>] {
        &self.nodes
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Stable debug snapshot for generated box-tree structure.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "box-tree").expect("write snapshot");
        writeln!(&mut out, "root: {}", box_id_debug_label(self.root)).expect("write snapshot");
        writeln!(&mut out, "boxes: {}", self.len()).expect("write snapshot");
        append_box_node_snapshot(&mut out, self, self.root, 0);
        out
    }
}

struct BoxTreeBuilder<'style_tree, 'dom> {
    nodes: Vec<BoxNode<'style_tree, 'dom>>,
}

impl<'style_tree, 'dom> BoxTreeBuilder<'style_tree, 'dom> {
    fn build_styled_subtree(
        &mut self,
        styled: &'style_tree StyledNode<'dom>,
        parent: Option<BoxId>,
        replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    ) -> Option<BoxId> {
        if matches!(styled.node, Node::Comment { .. }) {
            return None;
        }

        if matches!(styled.node, Node::Element { .. }) && styled.style.display() == Display::None {
            return None;
        }

        let replaced_kind = classify_replaced_kind(styled.node);
        let role = box_generation_role(styled, parent.map(|id| self.node(id).role()));
        let id = self.push_dom_backed_box(styled, parent, role, replaced_kind, replaced_info);

        if matches!(styled.node, Node::Document { .. } | Node::Element { .. }) {
            let (is_ul, is_ol) = list_container_kind(styled.node);
            let mut next_ol_index: u32 = 1;

            for child in &styled.children {
                let Some(child_id) = self.build_styled_subtree(child, Some(id), replaced_info)
                else {
                    continue;
                };

                if matches!(child.node, Node::Element { .. })
                    && self.node(child_id).display == Display::ListItem
                {
                    if is_ul {
                        self.node_mut(child_id).list_marker = Some(ListMarker::Unordered);
                    } else if is_ol {
                        self.node_mut(child_id).list_marker =
                            Some(ListMarker::Ordered(next_ol_index));
                        next_ol_index += 1;
                    }
                }

                self.node_mut(id).children.push(child_id);
            }
        }

        Some(id)
    }

    fn push_dom_backed_box(
        &mut self,
        styled: &'style_tree StyledNode<'dom>,
        parent: Option<BoxId>,
        role: BoxGenerationRole,
        replaced_kind: Option<ReplacedKind>,
        replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    ) -> BoxId {
        let kind = box_kind_for_styled_node(styled, role, replaced_kind);
        let replaced = if matches!(kind, BoxKind::ReplacedInline) {
            debug_assert!(replaced_kind.is_some());
            replaced_kind
        } else {
            None
        };
        let replaced_intrinsic = match replaced_kind {
            Some(ReplacedKind::Img) => replaced_info.and_then(|p| p.intrinsic_for_img(styled.node)),
            _ => None,
        };

        let id = BoxId(self.nodes.len());
        self.nodes.push(BoxNode {
            id,
            parent,
            children: Vec::new(),
            role,
            kind,
            source: BoxSource::DomNode(styled),
            style: &styled.style,
            display: styled.style.display(),
            list_marker: None,
            replaced,
            replaced_intrinsic,
        });
        id
    }

    fn push_fallback_root(&mut self, styled: &'style_tree StyledNode<'dom>) -> BoxId {
        let id = BoxId(self.nodes.len());
        self.nodes.push(BoxNode {
            id,
            parent: None,
            children: Vec::new(),
            role: BoxGenerationRole::DocumentRoot,
            kind: BoxKind::Block,
            source: BoxSource::DomNode(styled),
            style: &styled.style,
            display: styled.style.display(),
            list_marker: None,
            replaced: None,
            replaced_intrinsic: None,
        });
        id
    }

    fn node(&self, id: BoxId) -> &BoxNode<'style_tree, 'dom> {
        &self.nodes[id.index()]
    }

    fn node_mut(&mut self, id: BoxId) -> &mut BoxNode<'style_tree, 'dom> {
        &mut self.nodes[id.index()]
    }
}

fn box_generation_role(
    styled: &StyledNode<'_>,
    parent_role: Option<BoxGenerationRole>,
) -> BoxGenerationRole {
    match styled.node {
        Node::Document { .. } => BoxGenerationRole::DocumentRoot,
        Node::Element { .. } => {
            if parent_role == Some(BoxGenerationRole::DocumentRoot) {
                BoxGenerationRole::DocumentElement
            } else {
                BoxGenerationRole::OrdinaryElement
            }
        }
        Node::Text { .. } => BoxGenerationRole::TextRun,
        Node::Comment { .. } => unreachable!("comments do not generate boxes"),
    }
}

fn box_kind_for_styled_node(
    styled: &StyledNode<'_>,
    role: BoxGenerationRole,
    replaced_kind: Option<ReplacedKind>,
) -> BoxKind {
    let style = &styled.style;
    match role {
        BoxGenerationRole::DocumentRoot
        | BoxGenerationRole::DocumentElement
        | BoxGenerationRole::TextRun
        | BoxGenerationRole::Anonymous(_)
        | BoxGenerationRole::Marker => BoxKind::Block,
        BoxGenerationRole::OrdinaryElement => {
            if replaced_kind.is_some()
                && matches!(style.display(), Display::Inline | Display::InlineBlock)
            {
                BoxKind::ReplacedInline
            } else {
                match style.display() {
                    Display::Inline => BoxKind::Inline,
                    Display::InlineBlock => BoxKind::InlineBlock,
                    _ => BoxKind::Block,
                }
            }
        }
    }
}

fn list_container_kind(node: &Node) -> (bool, bool) {
    match node {
        Node::Element { name, .. } => {
            let is_ul = name.eq_ignore_ascii_case("ul");
            let is_ol = name.eq_ignore_ascii_case("ol");
            (is_ul, is_ol)
        }
        _ => (false, false),
    }
}

fn append_box_node_snapshot(out: &mut String, tree: &BoxTree<'_, '_>, id: BoxId, depth: usize) {
    let node = tree.node(id);
    let indent = "  ".repeat(depth);
    writeln!(
        out,
        "{indent}{}: parent={} source-id={} source={} role={} kind={} display={} children={} marker={} replaced={} intrinsic={}",
        box_id_debug_label(node.id),
        optional_box_id_debug_label(node.parent),
        optional_node_id_debug_label(node.direct_node_id()),
        node_debug_label(node.source.anchor_styled_node().node),
        role_debug_label(node.role),
        box_kind_debug_label(node.kind),
        display_debug_label(node.display),
        children_debug_label(&node.children),
        list_marker_debug_label(node.list_marker),
        replaced_kind_debug_label(node.replaced),
        intrinsic_size_debug_label(node.replaced_intrinsic),
    )
    .expect("write snapshot");

    for child in &node.children {
        append_box_node_snapshot(out, tree, *child, depth + 1);
    }
}

fn box_id_debug_label(id: BoxId) -> String {
    format!("b{}", id.index())
}

fn optional_box_id_debug_label(id: Option<BoxId>) -> String {
    id.map(box_id_debug_label)
        .unwrap_or_else(|| "none".to_string())
}

fn optional_node_id_debug_label(id: Option<Id>) -> String {
    id.map(|id| id.0.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn children_debug_label(children: &[BoxId]) -> String {
    let mut out = String::from("[");
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&box_id_debug_label(*child));
    }
    out.push(']');
    out
}

fn role_debug_label(role: BoxGenerationRole) -> &'static str {
    match role {
        BoxGenerationRole::DocumentRoot => "document-root",
        BoxGenerationRole::DocumentElement => "document-element",
        BoxGenerationRole::OrdinaryElement => "ordinary-element",
        BoxGenerationRole::TextRun => "text-run",
        BoxGenerationRole::Anonymous(AnonymousBoxKind::Block) => "anonymous-block",
        BoxGenerationRole::Anonymous(AnonymousBoxKind::Inline) => "anonymous-inline",
        BoxGenerationRole::Marker => "marker",
    }
}

fn display_debug_label(display: Display) -> &'static str {
    match display {
        Display::Block => "block",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
        Display::ListItem => "list-item",
        Display::None => "none",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use html::Node;
    use html::internal::Id;
    use std::sync::Arc;

    fn element(id: u32, name: &str, style: Vec<(&str, &str)>, children: Vec<Node>) -> Node {
        Node::Element {
            id: Id(id),
            name: Arc::from(name),
            attributes: Vec::new(),
            style: style
                .into_iter()
                .map(|(property, value)| (property.to_string(), value.to_string()))
                .collect(),
            children,
        }
    }

    fn text(id: u32, value: &str) -> Node {
        Node::Text {
            id: Id(id),
            text: value.to_string(),
        }
    }

    fn comment(id: u32, value: &str) -> Node {
        Node::Comment {
            id: Id(id),
            text: value.to_string(),
        }
    }

    fn doc(children: Vec<Node>) -> Node {
        Node::Document {
            id: Id(1),
            doctype: None,
            children,
        }
    }

    fn source_ids(tree: &BoxTree<'_, '_>) -> Vec<Option<Id>> {
        tree.nodes()
            .iter()
            .map(|node| node.direct_node_id())
            .collect()
    }

    #[test]
    fn box_tree_records_parent_child_links_in_deterministic_preorder() {
        let dom = doc(vec![element(
            2,
            "html",
            Vec::new(),
            vec![element(
                3,
                "body",
                Vec::new(),
                vec![element(
                    4,
                    "div",
                    Vec::new(),
                    vec![element(5, "span", Vec::new(), vec![text(6, "hello")])],
                )],
            )],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        assert_eq!(tree.root_id(), BoxId(0));
        assert_eq!(
            source_ids(&tree),
            vec![
                Some(Id(1)),
                Some(Id(2)),
                Some(Id(3)),
                Some(Id(4)),
                Some(Id(5)),
                Some(Id(6)),
            ]
        );

        for node in tree.nodes() {
            for child in node.children() {
                assert_eq!(tree.node(*child).parent(), Some(node.id()));
            }
        }

        assert_eq!(tree.node(BoxId(0)).children(), &[BoxId(1)]);
        assert_eq!(
            tree.node(BoxId(1)).role(),
            BoxGenerationRole::DocumentElement
        );
        assert_eq!(tree.node(BoxId(5)).role(), BoxGenerationRole::TextRun);
    }

    #[test]
    fn nested_html_element_is_not_classified_as_document_element() {
        let dom = doc(vec![element(
            2,
            "html",
            Vec::new(),
            vec![element(
                3,
                "body",
                Vec::new(),
                vec![element(4, "html", Vec::new(), Vec::new())],
            )],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        assert_eq!(
            tree.node(BoxId(1)).role(),
            BoxGenerationRole::DocumentElement
        );

        let nested_html = tree
            .nodes()
            .iter()
            .find(|node| node.direct_node_id() == Some(Id(4)))
            .expect("nested html box");
        assert_eq!(nested_html.role(), BoxGenerationRole::OrdinaryElement);
    }

    #[test]
    fn display_none_subtrees_are_omitted_from_box_tree() {
        let dom = doc(vec![element(
            2,
            "div",
            Vec::new(),
            vec![
                element(
                    3,
                    "span",
                    vec![("display", "none")],
                    vec![text(4, "hidden")],
                ),
                element(5, "span", Vec::new(), vec![text(6, "visible")]),
            ],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        assert_eq!(
            source_ids(&tree),
            vec![Some(Id(1)), Some(Id(2)), Some(Id(5)), Some(Id(6))]
        );
        assert!(
            tree.nodes()
                .iter()
                .all(|node| node.direct_node_id() != Some(Id(3)))
        );
        assert!(
            tree.nodes()
                .iter()
                .all(|node| node.direct_node_id() != Some(Id(4)))
        );
    }

    #[test]
    fn comments_do_not_generate_layout_boxes() {
        let dom = doc(vec![element(
            2,
            "div",
            Vec::new(),
            vec![comment(3, "ignored"), text(4, "visible")],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        assert_eq!(
            source_ids(&tree),
            vec![Some(Id(1)), Some(Id(2)), Some(Id(4))]
        );
        assert!(
            tree.nodes()
                .iter()
                .all(|node| node.direct_node_id() != Some(Id(3)))
        );
    }

    #[test]
    fn list_item_marker_metadata_is_assigned_from_box_tree_parent_context() {
        let dom = doc(vec![
            element(
                2,
                "ul",
                Vec::new(),
                vec![
                    element(3, "li", Vec::new(), vec![text(4, "a")]),
                    element(5, "li", Vec::new(), vec![text(6, "b")]),
                ],
            ),
            element(
                7,
                "ol",
                Vec::new(),
                vec![
                    element(8, "li", Vec::new(), vec![text(9, "one")]),
                    element(10, "li", Vec::new(), vec![text(11, "two")]),
                ],
            ),
        ]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        let markers = tree
            .nodes()
            .iter()
            .filter_map(|node| {
                node.list_marker()
                    .map(|marker| (node.direct_node_id(), marker))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            markers,
            vec![
                (Some(Id(3)), ListMarker::Unordered),
                (Some(Id(5)), ListMarker::Unordered),
                (Some(Id(8)), ListMarker::Ordered(1)),
                (Some(Id(10)), ListMarker::Ordered(2)),
            ]
        );
    }

    #[test]
    fn box_tree_stores_layout_metadata_without_dom_parent_ownership() {
        let dom = doc(vec![element(
            2,
            "div",
            Vec::new(),
            vec![
                element(3, "span", vec![("display", "inline-block")], Vec::new()),
                element(4, "input", Vec::new(), Vec::new()),
            ],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        let inline_block = tree
            .nodes()
            .iter()
            .find(|node| node.direct_node_id() == Some(Id(3)))
            .expect("inline-block box");
        assert_eq!(inline_block.kind(), BoxKind::InlineBlock);
        assert_eq!(inline_block.parent(), Some(BoxId(1)));
        assert_eq!(inline_block.source().direct_node_id(), Some(Id(3)));

        let input = tree
            .nodes()
            .iter()
            .find(|node| node.direct_node_id() == Some(Id(4)))
            .expect("input box");
        assert_eq!(input.replaced(), Some(ReplacedKind::InputText));
        assert_eq!(input.kind(), BoxKind::ReplacedInline);
        assert_eq!(input.parent(), Some(BoxId(1)));
    }

    #[test]
    fn box_tree_debug_snapshot_is_stable_and_structural() {
        let dom = doc(vec![element(
            2,
            "div",
            Vec::new(),
            vec![element(3, "span", Vec::new(), vec![text(4, "x")])],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        assert_eq!(
            tree.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "box-tree\n",
                "root: b0\n",
                "boxes: 4\n",
                "b0: parent=none source-id=1 source=document role=document-root kind=block display=inline children=[b1] marker=none replaced=none intrinsic=none\n",
                "  b1: parent=b0 source-id=2 source=element(\"div\") role=document-element kind=block display=block children=[b2] marker=none replaced=none intrinsic=none\n",
                "    b2: parent=b1 source-id=3 source=element(\"span\") role=ordinary-element kind=inline display=inline children=[b3] marker=none replaced=none intrinsic=none\n",
                "      b3: parent=b2 source-id=4 source=text(\"x\") role=text-run kind=block display=inline children=[] marker=none replaced=none intrinsic=none\n",
            )
        );
    }

    #[test]
    fn box_source_can_represent_future_non_dom_backed_boxes() {
        let dom = doc(vec![element(2, "div", Vec::new(), Vec::new())]);
        let styled = css::build_style_tree(&dom, None);
        let div = &styled.children[0];
        let source = BoxSource::Anonymous {
            parent: div,
            kind: AnonymousBoxKind::Block,
        };

        assert_eq!(source.direct_node_id(), None);
        assert!(source.direct_styled_node().is_none());
        assert_eq!(source.anchor_node_id(), Id(2));
        assert_eq!(source.anchor_styled_node().node_id, Id(2));
    }
}
