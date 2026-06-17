use std::fmt::Write;

use css::ZIndex;
use html::dom_utils::is_non_rendering_element;
use layout::{LayoutBox, PositioningScheme};

use super::PaintSource;

/// Frame-local identity for a paint-owned stacking context.
///
/// IDs are assigned deterministically while building `StackingContextTree` from
/// the layout output. They are not compositor layer IDs, retained scene IDs, or
/// backend resource handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackingContextId(usize);

impl StackingContextId {
    pub const ROOT: Self = Self(0);

    pub fn index(self) -> usize {
        self.0
    }
}

/// Paint-owned, frame-local stacking context representation.
///
/// AB2 introduced the deterministic root stacking context. AB3 refines that
/// representation with paint-owned child contexts only for positioned generated
/// boxes with computed integer `z-index`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackingContextTree {
    root: StackingContextId,
    contexts: Vec<StackingContextNode>,
}

impl StackingContextTree {
    pub fn from_layout(root: &LayoutBox<'_, '_>) -> Self {
        let root_source = PaintSource::from_layout(root);
        let root_node = StackingContextNode {
            id: StackingContextId::ROOT,
            parent: None,
            source: StackingContextSource::RootDocument(root_source),
            order_key: StackingOrderKey::root(),
            children: Vec::new(),
            items: Vec::new(),
        };

        let mut tree = Self {
            root: StackingContextId::ROOT,
            contexts: vec![root_node],
        };
        let mut next_tree_order = 0;
        collect_context_contents(
            root,
            StackingContextId::ROOT,
            &mut tree,
            &mut next_tree_order,
        );

        tree
    }

    pub fn root_id(&self) -> StackingContextId {
        self.root
    }

    pub fn root(&self) -> &StackingContextNode {
        &self.contexts[self.root.index()]
    }

    pub fn contexts(&self) -> &[StackingContextNode] {
        &self.contexts
    }

    pub fn context(&self, id: StackingContextId) -> Option<&StackingContextNode> {
        self.contexts.get(id.index())
    }

    pub fn context_for_source(&self, source: PaintSource) -> Option<StackingContextId> {
        self.contexts.iter().find_map(|context| {
            context
                .items
                .iter()
                .any(|item| item.source == source)
                .then_some(context.id)
        })
    }

    pub fn context_id_for_source_context(&self, source: PaintSource) -> Option<StackingContextId> {
        self.contexts
            .iter()
            .find_map(|context| (context.source.paint_source() == source).then_some(context.id))
    }

    pub fn child_contexts_for_layer(
        &self,
        parent: StackingContextId,
        layer: StackingLayerKind,
    ) -> Vec<&StackingContextNode> {
        let mut children = self
            .context(parent)
            .map(|context| {
                context
                    .children
                    .iter()
                    .filter_map(|child| self.context(*child))
                    .filter(|child| child.order_key.layer == layer)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        children.sort_by_key(|context| context.order_key);
        children
    }

    /// Returns the canonical paint-owned order for one stacking context.
    ///
    /// AB4 makes this slot order the shared source of cross-context paint
    /// ordering for semantic order snapshots, operation snapshots, and
    /// immediate painting. The context source subtree is represented as a slot
    /// so consumers can keep AA per-box ordering inside the source while
    /// emitting child contexts only through their explicit stacking slots.
    pub fn ordered_slots(&self, context: StackingContextId) -> Vec<StackingOrderSlot> {
        let mut slots = Vec::new();

        self.push_child_context_slots(context, StackingLayerKind::NegativeZIndex, &mut slots);

        if let Some(context_node) = self.context(context) {
            slots.push(StackingOrderSlot::ContextSource(
                context_node.source.paint_source(),
            ));
        }

        self.push_child_context_slots(context, StackingLayerKind::ZeroZIndex, &mut slots);
        self.push_child_context_slots(context, StackingLayerKind::PositiveZIndex, &mut slots);

        slots
    }

    pub fn source_starts_external_context(
        &self,
        owner_context: StackingContextId,
        source: PaintSource,
    ) -> bool {
        self.context_id_for_source_context(source)
            .is_some_and(|context| context != owner_context)
    }

    fn push_child_context_slots(
        &self,
        parent: StackingContextId,
        layer: StackingLayerKind,
        slots: &mut Vec<StackingOrderSlot>,
    ) {
        slots.extend(
            self.child_contexts_for_layer(parent, layer)
                .into_iter()
                .map(|context| StackingOrderSlot::ChildContext(context.id)),
        );
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 2").expect("write stacking context snapshot");
        writeln!(&mut out, "stacking-context-tree").expect("write stacking context snapshot");
        writeln!(&mut out, "root-context: {}", self.root.index())
            .expect("write stacking context snapshot");
        for context in &self.contexts {
            context
                .append_debug_snapshot(&mut out, 0)
                .expect("write stacking context snapshot");
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackingContextNode {
    id: StackingContextId,
    parent: Option<StackingContextId>,
    source: StackingContextSource,
    order_key: StackingOrderKey,
    children: Vec<StackingContextId>,
    items: Vec<StackablePaintItem>,
}

impl StackingContextNode {
    pub fn id(&self) -> StackingContextId {
        self.id
    }

    pub fn parent(&self) -> Option<StackingContextId> {
        self.parent
    }

    pub fn source(&self) -> StackingContextSource {
        self.source
    }

    pub fn order_key(&self) -> StackingOrderKey {
        self.order_key
    }

    pub fn children(&self) -> &[StackingContextId] {
        &self.children
    }

    pub fn items(&self) -> &[StackablePaintItem] {
        &self.items
    }

    fn append_debug_snapshot(&self, out: &mut String, depth: usize) -> std::fmt::Result {
        let indent = "  ".repeat(depth);
        writeln!(
            out,
            "{}context id={} parent={} source={} layer={} z-index={} tree-order={} children={} items={}",
            indent,
            self.id.index(),
            optional_context_debug_label(self.parent),
            self.source.to_debug_label(),
            self.order_key.layer.debug_label(),
            optional_z_index_debug_label(self.order_key.z_index),
            self.order_key.tree_order,
            self.children.len(),
            self.items.len()
        )?;
        for item in &self.items {
            writeln!(
                out,
                "{}  item source={} context={} layer={} z-index={} tree-order={}",
                indent,
                paint_source_debug_label(item.source),
                item.context.index(),
                item.order_key.layer.debug_label(),
                optional_z_index_debug_label(item.order_key.z_index),
                item.order_key.tree_order
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackingContextSource {
    RootDocument(PaintSource),
    PositionedElement(PaintSource),
}

impl StackingContextSource {
    pub fn paint_source(self) -> PaintSource {
        match self {
            Self::RootDocument(source) => source,
            Self::PositionedElement(source) => source,
        }
    }

    fn to_debug_label(self) -> String {
        match self {
            Self::RootDocument(source) => {
                format!("root-document({})", paint_source_debug_label(source))
            }
            Self::PositionedElement(source) => {
                format!("positioned-element({})", paint_source_debug_label(source))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackablePaintItem {
    source: PaintSource,
    context: StackingContextId,
    order_key: StackingOrderKey,
}

impl StackablePaintItem {
    pub fn source(self) -> PaintSource {
        self.source
    }

    pub fn context(self) -> StackingContextId {
        self.context
    }

    pub fn order_key(self) -> StackingOrderKey {
        self.order_key
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackingOrderSlot {
    ChildContext(StackingContextId),
    ContextSource(PaintSource),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StackingLayerKind {
    NegativeZIndex,
    NormalFlow,
    ZeroZIndex,
    PositiveZIndex,
}

impl StackingLayerKind {
    pub fn debug_label(self) -> &'static str {
        match self {
            Self::NegativeZIndex => "negative-z-index",
            Self::NormalFlow => "normal-flow",
            Self::ZeroZIndex => "zero-z-index",
            Self::PositiveZIndex => "positive-z-index",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StackingOrderKey {
    layer: StackingLayerKind,
    z_index: Option<i32>,
    tree_order: usize,
}

impl StackingOrderKey {
    fn root() -> Self {
        Self {
            layer: StackingLayerKind::NormalFlow,
            z_index: None,
            tree_order: 0,
        }
    }

    fn normal_flow(tree_order: usize) -> Self {
        Self {
            layer: StackingLayerKind::NormalFlow,
            z_index: None,
            tree_order,
        }
    }

    fn from_z_index(z_index: i32, tree_order: usize) -> Self {
        let layer = match z_index.cmp(&0) {
            std::cmp::Ordering::Less => StackingLayerKind::NegativeZIndex,
            std::cmp::Ordering::Equal => StackingLayerKind::ZeroZIndex,
            std::cmp::Ordering::Greater => StackingLayerKind::PositiveZIndex,
        };

        Self {
            layer,
            z_index: Some(z_index),
            tree_order,
        }
    }

    pub fn layer(self) -> StackingLayerKind {
        self.layer
    }

    pub fn z_index(self) -> Option<i32> {
        self.z_index
    }

    pub fn tree_order(self) -> usize {
        self.tree_order
    }
}

fn collect_context_contents(
    layout: &LayoutBox<'_, '_>,
    current_context: StackingContextId,
    tree: &mut StackingContextTree,
    next_tree_order: &mut usize,
) {
    if is_non_rendering_element(layout.node.node) {
        return;
    }

    let tree_order = *next_tree_order;
    *next_tree_order += 1;
    let source = PaintSource::from_layout(layout);

    if source != tree.contexts[current_context.index()].source.paint_source()
        && let Some(z_index) = ab3_child_context_z_index(layout)
    {
        push_child_context(
            current_context,
            layout,
            z_index,
            tree_order,
            tree,
            next_tree_order,
        );
        return;
    }

    if current_context != StackingContextId::ROOT
        || source != tree.root().source.paint_source()
        || !creates_ab3_child_stacking_context(layout)
    {
        tree.contexts[current_context.index()]
            .items
            .push(StackablePaintItem {
                source,
                context: current_context,
                order_key: StackingOrderKey::normal_flow(tree_order),
            });
    }

    for child in &layout.children {
        if let Some(z_index) = ab3_child_context_z_index(child) {
            let child_tree_order = *next_tree_order;
            *next_tree_order += 1;
            push_child_context(
                current_context,
                child,
                z_index,
                child_tree_order,
                tree,
                next_tree_order,
            );
        } else {
            collect_context_contents(child, current_context, tree, next_tree_order);
        }
    }
}

fn push_child_context(
    parent_context: StackingContextId,
    layout: &LayoutBox<'_, '_>,
    z_index: i32,
    tree_order: usize,
    tree: &mut StackingContextTree,
    next_tree_order: &mut usize,
) {
    let child_id = StackingContextId(tree.contexts.len());
    let child_source = PaintSource::from_layout(layout);
    tree.contexts[parent_context.index()]
        .children
        .push(child_id);
    tree.contexts.push(StackingContextNode {
        id: child_id,
        parent: Some(parent_context),
        source: StackingContextSource::PositionedElement(child_source),
        order_key: StackingOrderKey::from_z_index(z_index, tree_order),
        children: Vec::new(),
        items: vec![StackablePaintItem {
            source: child_source,
            context: child_id,
            order_key: StackingOrderKey::normal_flow(tree_order),
        }],
    });

    for child in &layout.children {
        collect_context_contents(child, child_id, tree, next_tree_order);
    }
}

fn creates_ab3_child_stacking_context(layout: &LayoutBox<'_, '_>) -> bool {
    ab3_child_context_z_index(layout).is_some()
}

fn ab3_child_context_z_index(layout: &LayoutBox<'_, '_>) -> Option<i32> {
    if matches!(layout.positioning_scheme(), PositioningScheme::Static) {
        return None;
    }

    match layout.style.z_index() {
        ZIndex::Auto => None,
        ZIndex::Integer(value) => Some(value),
    }
}

fn optional_context_debug_label(id: Option<StackingContextId>) -> String {
    match id {
        Some(id) => id.index().to_string(),
        None => "none".to_string(),
    }
}

fn optional_z_index_debug_label(z_index: Option<i32>) -> String {
    match z_index {
        Some(value) => value.to_string(),
        None => "auto".to_string(),
    }
}

fn paint_source_debug_label(source: PaintSource) -> String {
    format!(
        "box={} node={} anonymous={}",
        source.box_id, source.node_id.0, source.anonymous
    )
}
