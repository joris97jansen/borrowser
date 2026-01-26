pub mod collect;
pub mod debug;
pub mod dom_diff;
#[cfg(any(test, feature = "dom-snapshot"))]
pub mod dom_snapshot;
pub mod dom_utils;
pub mod golden_corpus;
pub mod head;
#[cfg(test)]
mod streaming_parity;
#[cfg(test)]
pub mod test_harness;
pub mod traverse;

mod dom_builder;
mod dom_patch;
mod entities;
mod tokenizer;
mod types;

use memchr::{memchr, memchr2};

pub fn is_html(ct: &Option<String>) -> bool {
    let Some(value) = ct.as_deref() else {
        return false;
    };
    contains_ignore_ascii_case(value, b"text/html")
        || contains_ignore_ascii_case(value, b"application/xhtml")
}

fn contains_ignore_ascii_case(haystack: &str, needle: &[u8]) -> bool {
    let hay = haystack.as_bytes();
    let n = needle.len();
    if n == 0 {
        return true;
    }
    let hay_len = hay.len();
    if hay_len < n {
        return false;
    }
    let first = needle[0];
    let (a, b) = if first.is_ascii_alphabetic() {
        (first.to_ascii_lowercase(), first.to_ascii_uppercase())
    } else {
        (first, first)
    };
    if n == 1 {
        if a == b {
            return memchr(a, hay).is_some();
        }
        return memchr2(a, b, hay).is_some();
    }
    let mut i = 0;
    while i + n <= hay_len {
        let rel = if a == b {
            memchr(a, &hay[i..])
        } else {
            memchr2(a, b, &hay[i..])
        };
        let Some(rel) = rel else {
            return false;
        };
        let pos = i + rel;
        if pos + n <= hay_len && hay[pos..pos + n].eq_ignore_ascii_case(needle) {
            return true;
        }
        i = pos + 1;
    }
    false
}

pub use crate::dom_builder::{
    PatchEmitter, PatchEmitterHandle, TokenTextResolver, TreeBuilder, TreeBuilderConfig,
    TreeBuilderError, TreeBuilderResult,
};
pub use crate::dom_builder::build_dom;
pub use crate::dom_diff::{DomDiffState, diff_dom, diff_dom_with_state, diff_from_empty};
pub use crate::dom_patch::{DomPatch, PatchKey};
pub use crate::tokenizer::Tokenizer;
pub use crate::tokenizer::tokenize;
pub use crate::types::{AtomId, AtomTable, Node, Token, TokenStream};

#[cfg(feature = "internal-api")]
pub mod internal {
    pub use super::types::{Id, NodeId, NodeKey};
}

#[cfg(all(test, feature = "count-alloc"))]
mod test_alloc {
    // Counters are intentionally lightweight: they measure allocation events and growth bytes
    // while enabled, not current live heap usage.
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    pub struct CountingAlloc;

    static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
    static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

    static ENABLED: AtomicBool = AtomicBool::new(false);

    unsafe impl GlobalAlloc for CountingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = System.alloc(layout);
            if !ptr.is_null() {
                if ENABLED.load(Ordering::Relaxed) {
                    ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
                    ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
                }
            }
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let ptr = System.alloc_zeroed(layout);
            if !ptr.is_null() {
                if ENABLED.load(Ordering::Relaxed) {
                    ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
                    ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
                }
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout);
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let new_ptr = System.realloc(ptr, layout, new_size);
            if !new_ptr.is_null() {
                if ENABLED.load(Ordering::Relaxed) {
                    ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
                    let old_size = layout.size();
                    if new_size > old_size {
                        ALLOC_BYTES.fetch_add(new_size - old_size, Ordering::Relaxed);
                    }
                }
            }
            new_ptr
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAlloc = CountingAlloc;

    pub fn reset() {
        ALLOC_COUNT.store(0, Ordering::Relaxed);
        ALLOC_BYTES.store(0, Ordering::Relaxed);
        ENABLED.store(false, Ordering::Relaxed);
    }

    pub fn enable() {
        ENABLED.store(true, Ordering::Relaxed);
    }

    pub fn disable() {
        ENABLED.store(false, Ordering::Relaxed);
    }

    pub fn counts() -> (usize, usize) {
        (
            ALLOC_COUNT.load(Ordering::Relaxed),
            ALLOC_BYTES.load(Ordering::Relaxed),
        )
    }

    pub struct AllocGuard;

    impl AllocGuard {
        pub fn new() -> Self {
            reset();
            enable();
            Self
        }
    }

    impl Drop for AllocGuard {
        fn drop(&mut self) {
            disable();
        }
    }
}
