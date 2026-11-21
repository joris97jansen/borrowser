//! Isolated AI8 allocation-work probe; no sockets, timing threshold or new API.
//! Like the existing CSS allocation guards, count only the measuring thread.
//! Count full successful allocation/reallocation requests, not peak/live memory
//! or exact bytes copied by the allocator. Inputs are built before measurement.
use addressing::{BrowserUrl, ResourceAddress, ResourceKind};
use net::{HttpHeaders, HttpStatus, NetworkResponseInfo, ResponseContentType};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};

struct CountingAllocator;

thread_local! {
    static REQUESTED_BYTES: Cell<Option<usize>> = const { Cell::new(None) };
}

fn record(bytes: usize) {
    let _ = REQUESTED_BYTES.try_with(|counter| {
        if let Some(total) = counter.get() {
            counter.set(Some(total.saturating_add(bytes)));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record(layout.size());
        }
        pointer
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(pointer, layout, size) };
        if !pointer.is_null() {
            record(size);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn ai8_discarded_mime_candidates_do_not_reallocate_the_large_run_charset() {
    let mut headers = HttpHeaders::new();
    let mut first = vec![0xff; 8000];
    first[..13].copy_from_slice(b"a/a;charset=\"");
    headers.append("content-type", &first).unwrap();
    for _ in 0..2 {
        headers.append("content-type", &vec![0xff; 8000]).unwrap();
    }
    let mut fourth = vec![0xff; 8000];
    fourth[7999] = b'"';
    headers.append("content-type", &fourth).unwrap();
    for _ in 0..4 {
        headers
            .append("content-type", &b"a/a,".repeat(2000))
            .unwrap();
    }
    let accounted: usize = headers
        .iter()
        .map(|(name, value)| name.len() + value.len() + 4)
        .sum();
    assert_eq!(accounted, 64_128);
    let address = ResourceAddress::new(
        BrowserUrl::parse("https://probe.invalid/").unwrap().url,
        ResourceKind::Document,
    );
    let issued = address.url().as_str().to_owned();
    let reported = Some(issued.clone());
    let status = HttpStatus::try_from(200).unwrap();

    REQUESTED_BYTES.with(|counter| counter.set(Some(0)));
    let response = black_box(NetworkResponseInfo::http(
        black_box(&address),
        issued,
        reported,
        status,
        black_box(headers),
    ));
    let allocated = REQUESTED_BYTES.with(|counter| counter.replace(None).unwrap());

    let response = response.unwrap();
    let ResponseContentType::Parsed(mime) = response.content_type() else {
        panic!("selected MIME");
    };
    assert_eq!(mime.mime_type(), "a/a");
    assert_eq!(black_box(mime.charset().unwrap()).len(), 63_978);
    // Generous work envelope for logical combination, byte-isomorphic parsing,
    // response facts and 8,000 tiny MIME candidates. It is a test-only guard,
    // not a production declaration limit. The previous loop alone copied
    // 511,824,000 inherited-charset bytes and fails this 2,052,096-byte envelope.
    let budget = 32 * accounted;
    eprintln!("AI8 MIME allocation probe: requested_bytes={allocated} budget={budget}");
    assert!(
        allocated <= budget,
        "repeated charset allocation work: {allocated} > {budget}"
    );
}
