pub mod collect;
pub mod debug;
pub mod dom_utils;
pub mod head;
pub mod traverse;

mod dom_builder;
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
    if hay.len() < n {
        return false;
    }
    let first = needle[0];
    let (a, b) = if first.is_ascii_alphabetic() {
        (first.to_ascii_lowercase(), first.to_ascii_uppercase())
    } else {
        (first, first)
    };
    let mut i = 0;
    while i + n <= hay.len() {
        let rel = if a == b {
            memchr(a, &hay[i..])
        } else {
            memchr2(a, b, &hay[i..])
        };
        let Some(rel) = rel else {
            return false;
        };
        let pos = i + rel;
        if pos + n <= hay.len() && hay[pos..pos + n].eq_ignore_ascii_case(needle) {
            return true;
        }
        i = pos + 1;
    }
    false
}

pub use crate::dom_builder::build_dom;
pub use crate::tokenizer::tokenize;
pub use crate::types::{AtomId, AtomTable, Id, Node, NodeId, Token, TokenStream};

#[cfg(all(test, feature = "count-alloc"))]
mod test_alloc {
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
}
