use super::super::{DomPatchError, DomStore};
use core_types::{DomHandle, DomVersion};
use html::{DomPatch, Node, PatchKey};

pub(super) fn handle(id: u64) -> DomHandle {
    DomHandle(id)
}

pub(super) fn new_store_with_handle(id: u64) -> (DomStore, DomHandle) {
    let mut store = DomStore::new();
    let handle = self::handle(id);
    store.create(handle).expect("create handle");
    (store, handle)
}

pub(super) struct VersionSteps {
    current: DomVersion,
}

impl VersionSteps {
    pub(super) fn new() -> Self {
        Self {
            current: DomVersion::INITIAL,
        }
    }

    pub(super) fn next_pair(&self) -> (DomVersion, DomVersion) {
        (self.current, self.current.next())
    }

    pub(super) fn commit(&mut self, next: DomVersion) {
        debug_assert_eq!(self.current.next(), next);
        self.current = next;
    }
}

pub(super) fn apply_ok(
    store: &mut DomStore,
    handle: DomHandle,
    versions: &mut VersionSteps,
    patches: &[DomPatch],
    context: &str,
) {
    let (from, to) = versions.next_pair();
    store.apply(handle, from, to, patches).expect(context);
    versions.commit(to);
}

pub(super) fn bootstrap_document(
    store: &mut DomStore,
    handle: DomHandle,
    versions: &mut VersionSteps,
    key: PatchKey,
) {
    apply_ok(
        store,
        handle,
        versions,
        &[DomPatch::CreateDocument { key, doctype: None }],
        "bootstrap apply",
    );
}

pub(super) fn assert_failed_apply_is_atomic(
    store: &mut DomStore,
    handle: DomHandle,
    from: DomVersion,
    to: DomVersion,
    patches: &[DomPatch],
) -> DomPatchError {
    let before = materialized_dom_lines(store, handle);
    let err = store
        .apply(handle, from, to, patches)
        .expect_err("apply should fail");
    let after = materialized_dom_lines(store, handle);
    assert_eq!(before, after, "failed batch must not partially commit");
    err
}

pub(super) fn materialized_dom_lines(store: &DomStore, handle: DomHandle) -> Vec<String> {
    let node = store
        .materialize(handle)
        .expect("materialize for snapshot should succeed");
    dom_snapshot_lines(&node)
}

fn dom_snapshot_lines(node: &Node) -> Vec<String> {
    fn escape(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '"' => out.push_str("\\\""),
                '<' => out.push_str("\\u{3C}"),
                '>' => out.push_str("\\u{3E}"),
                c if c.is_ascii_control() => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
                c if c.is_ascii() => out.push(c),
                c => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
            }
        }
        out
    }

    fn push_node(out: &mut Vec<String>, node: &Node, depth: usize) {
        let indent = "  ".repeat(depth);
        match node {
            Node::Document {
                doctype, children, ..
            } => {
                out.push(match doctype {
                    Some(doctype) => {
                        format!("{indent}#document doctype=\"{}\"", escape(doctype))
                    }
                    None => format!("{indent}#document doctype=<none>"),
                });
                for child in children {
                    push_node(out, child, depth + 1);
                }
            }
            Node::Element {
                name,
                attributes,
                children,
                ..
            } => {
                let mut attrs = attributes
                    .iter()
                    .map(|(key, value)| (key.as_ref(), value.as_deref()))
                    .collect::<Vec<_>>();
                attrs.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
                let attrs = attrs
                    .into_iter()
                    .map(|(key, value)| match value {
                        Some(value) => format!("{key}=\"{}\"", escape(value)),
                        None => format!("{key}=<none>"),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(format!("{indent}<{name} attrs=[{attrs}]>"));
                for child in children {
                    push_node(out, child, depth + 1);
                }
            }
            Node::Text { text, .. } => {
                out.push(format!("{indent}text=\"{}\"", escape(text)));
            }
            Node::Comment { text, .. } => {
                out.push(format!("{indent}comment=\"{}\"", escape(text)));
            }
        }
    }

    let mut out = Vec::new();
    push_node(&mut out, node, 0);
    out
}
