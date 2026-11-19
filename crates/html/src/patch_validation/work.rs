//! Executed validation work in a single synchronous unit-test scope.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Work {
    pub(crate) completion_batches: usize,
    pub(crate) completion_patches: usize,
    pub(crate) full_checks: usize,
    pub(crate) audit_nodes: usize,
    pub(crate) audit_edges: usize,
    pub(crate) reverse_comparisons: usize,
    pub(crate) document_children: usize,
    pub(crate) cycle_pops: usize,
    pub(crate) mutation_sibling_tests: usize,
}

thread_local! {
    static ACTIVE: Cell<Option<Work>> = const { Cell::new(None) };
}

pub(super) fn record(f: impl FnOnce(&mut Work)) {
    ACTIVE.with(|active| {
        if let Some(mut work) = active.get() {
            f(&mut work);
            active.set(Some(work));
        }
    });
}

pub(crate) fn measure<T>(f: impl FnOnce() -> T) -> (T, Work) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            ACTIVE.set(None);
        }
    }
    assert!(ACTIVE.get().is_none(), "nested validation-work measurement");
    ACTIVE.set(Some(Work::default()));
    let _scope = Scope;
    let result = f();
    (result, ACTIVE.get().expect("active validation-work scope"))
}
