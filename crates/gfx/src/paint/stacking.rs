use std::fmt::Write;

use html::dom_utils::is_non_rendering_element;
use layout::LayoutBox;

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
/// AB2 intentionally models only the root stacking context. All currently
/// paintable layout boxes are associated with that root context in stable
/// layout traversal order. This does not change visual paint ordering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackingContextTree {
    root: StackingContextId,
    contexts: Vec<StackingContextNode>,
}

impl StackingContextTree {
    pub fn from_layout(root: &LayoutBox<'_, '_>) -> Self {
        let root_source = PaintSource::from_layout(root);
        let mut root_node = StackingContextNode {
            id: StackingContextId::ROOT,
            parent: None,
            source: StackingContextSource::RootDocument(root_source),
            children: Vec::new(),
            items: Vec::new(),
        };

        collect_root_context_items(root, &mut root_node.items);

        Self {
            root: StackingContextId::ROOT,
            contexts: vec![root_node],
        }
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

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write stacking context snapshot");
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
            "{}context id={} parent={} source={} children={} items={}",
            indent,
            self.id.index(),
            optional_context_debug_label(self.parent),
            self.source.to_debug_label(),
            self.children.len(),
            self.items.len()
        )?;
        for item in &self.items {
            writeln!(
                out,
                "{}  item source={} context={}",
                indent,
                paint_source_debug_label(item.source),
                item.context.index()
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackingContextSource {
    RootDocument(PaintSource),
}

impl StackingContextSource {
    pub fn paint_source(self) -> PaintSource {
        match self {
            Self::RootDocument(source) => source,
        }
    }

    fn to_debug_label(self) -> String {
        match self {
            Self::RootDocument(source) => {
                format!("root-document({})", paint_source_debug_label(source))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackablePaintItem {
    source: PaintSource,
    context: StackingContextId,
}

impl StackablePaintItem {
    pub fn source(self) -> PaintSource {
        self.source
    }

    pub fn context(self) -> StackingContextId {
        self.context
    }
}

fn collect_root_context_items(layout: &LayoutBox<'_, '_>, items: &mut Vec<StackablePaintItem>) {
    if is_non_rendering_element(layout.node.node) {
        return;
    }

    items.push(StackablePaintItem {
        source: PaintSource::from_layout(layout),
        context: StackingContextId::ROOT,
    });

    for child in &layout.children {
        collect_root_context_items(child, items);
    }
}

fn optional_context_debug_label(id: Option<StackingContextId>) -> String {
    match id {
        Some(id) => id.index().to_string(),
        None => "none".to_string(),
    }
}

fn paint_source_debug_label(source: PaintSource) -> String {
    format!(
        "box={} node={} anonymous={}",
        source.box_id, source.node_id.0, source.anonymous
    )
}
